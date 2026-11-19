use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;

/// Shared effort estimate of one registered callback.
///
/// Holds the Exponential Moving Average of the measured real cost of the work the callback triggers,
/// in microseconds. The value can be updated from any thread once that work is finished.
pub(crate) struct CallbackControl {
    effort_us: AtomicU64,
    /// Period used when the callback is rescheduled, changeable at runtime.
    period_ms: AtomicU32,
}

impl CallbackControl {
    fn new(period_ms: u32) -> Self {
        Self {
            effort_us: AtomicU64::new(INITIAL_EFFORT_US),
            period_ms: AtomicU32::new(period_ms.max(1)),
        }
    }

    /// Current effort estimate in microseconds.
    #[must_use]
    pub(crate) fn effort_us(&self) -> u64 {
        self.effort_us.load(AtomicOrdering::Relaxed)
    }

    /// Feeds a freshly measured cost in microseconds into the estimate.
    pub(crate) fn update_effort(&self, measured_us: u64) {
        let old = self.effort_us.load(AtomicOrdering::Relaxed);
        let new = (EMA_NEW_WEIGHT * measured_us + (EMA_TOTAL_WEIGHT - EMA_NEW_WEIGHT) * old)
            / EMA_TOTAL_WEIGHT;
        // A racing update only drops one sample, which barely moves the average.
        self.effort_us.store(new, AtomicOrdering::Relaxed);
    }

    /// Changes the period used for the following reschedules.
    pub(crate) fn set_period(&self, period: Duration) {
        let period_ms = to_period_ms(period);
        self.period_ms.store(period_ms, AtomicOrdering::Relaxed);
    }

    fn period_ms(&self) -> u32 {
        self.period_ms.load(AtomicOrdering::Relaxed).max(1)
    }
}

/// Identifier of a registered callback, used to remove it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct CallbackHandle(u32);

/// Sorts callbacks by next deadline; the registration id breaks ties so equal
/// deadlines keep a stable, deterministic order.
#[derive(Clone, Copy, PartialEq, Eq)]
struct HeapEntry {
    deadline_ms: u32,
    id: u32,
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.deadline_ms, self.id).cmp(&(other.deadline_ms, other.id))
    }
}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

type Callback = Box<dyn FnMut(&Arc<CallbackControl>) + Send + Sync>;

struct Task {
    deadline_ms: u32,
    control: Arc<CallbackControl>,
    callback: Callback,
}

/// Runs registered callbacks on a per-frame work budget, so the total callback
/// cost varies little between frames.
///
/// Time is a virtual counter advanced only by [`execute`](Self::execute), so it
/// can be paused and never jumps on real-time hitches. Deadlines are this
/// virtual time in milliseconds, while effort is real execution time.
pub(crate) struct AmortizedExecutor {
    now_ms: u32,
    /// Sub-millisecond part of the real delta that was not yet added to
    /// `now_ms`, so truncating the frame delta does not slow the clock down.
    remainder_us: u32,
    budget_us: u64,
    max_budget_us: u64,
    unlimited: bool,
    next_id: u32,
    tasks: HashMap<u32, Task>,
    heap: BinaryHeap<Reverse<HeapEntry>>,
}

impl AmortizedExecutor {
    /// Executor that does not skip due work, used when replay determinism
    /// matters more than frame smoothing.
    #[must_use]
    pub(crate) fn new_unlimited() -> Self {
        Self {
            budget_us: u64::MAX,
            max_budget_us: u64::MAX,
            unlimited: true,
            ..Self::default()
        }
    }

    /// Registers `callback` to run every `period` of virtual time, rounded down
    /// to whole milliseconds (at least one).
    ///
    /// The first run is offset into `(period, 2 * period)` by a deterministic
    /// jitter, so callbacks registered together do not all become due at once.
    pub(crate) fn add(
        &mut self,
        period: Duration,
        callback: impl FnMut(&Arc<CallbackControl>) + Send + Sync + 'static,
    ) -> CallbackHandle {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let period_ms = to_period_ms(period);

        let control = Arc::new(CallbackControl::new(period_ms));
        let deadline_ms = self.now_ms + period_ms + jitter(id, period_ms);
        self.tasks.insert(
            id,
            Task {
                deadline_ms,
                control,
                callback: Box::new(callback),
            },
        );
        self.heap.push(Reverse(HeapEntry { deadline_ms, id }));
        CallbackHandle(id)
    }

    /// Removes a registered callback. Its heap entry is skipped when it comes
    /// due.
    pub(crate) fn remove(&mut self, handle: CallbackHandle) {
        self.tasks.remove(&handle.0);
    }

    /// Advances virtual time by `delta_us` and runs due callbacks until the
    /// frame budget is spent. At least one due callback always runs.
    pub(crate) fn execute(&mut self, delta_us: u64) {
        self.advance(delta_us);

        let budget_us = self.budget_us;
        let mut spent_us: u64 = 0;
        let mut called: u32 = 0;

        while let Some(Reverse(head)) = self.heap.peek() {
            if head.deadline_ms > self.now_ms {
                break;
            }
            if called > 0 && spent_us >= budget_us {
                break;
            }

            let Some(Reverse(head)) = self.heap.pop() else {
                break;
            };
            let Some(task) = self.tasks.get_mut(&head.id) else {
                continue;
            };

            let effort_us = task.control.effort_us();
            if effort_us > WARN_TASK_EFFORT {
                bevy::log::info!(
                    "There is task({}) with execution time over ({})us",
                    head.id,
                    WARN_TASK_EFFORT
                );
            }
            let period_ms = task.control.period_ms();
            let mut next_deadline_ms = head.deadline_ms.saturating_add(period_ms);
            if next_deadline_ms <= self.now_ms {
                // skip whole missed periods, keep the phase
                let behind = self.now_ms - next_deadline_ms;
                let skips = behind / period_ms + 1;
                next_deadline_ms = next_deadline_ms.saturating_add(skips.saturating_mul(period_ms));
            }
            (task.callback)(&task.control);

            task.deadline_ms = next_deadline_ms;
            self.heap.push(Reverse(HeapEntry {
                deadline_ms: next_deadline_ms,
                id: head.id,
            }));

            spent_us += effort_us;
            called += 1;
        }

        let budget_delta = budget_us as i64 - spent_us as i64;
        if !self.unlimited && budget_delta > WARN_TASK_OVERRUN as i64 {
            bevy::log::info!(
                "Overrun frame budget over {}us: tasks_num={} called={called} dt={budget_delta} budget_us={budget_us} used_us={spent_us}",
                WARN_TASK_OVERRUN,
                self.tasks.len()
            );
        }
        self.update_budget();
    }

    fn advance(&mut self, delta_us: u64) {
        let total_us = u64::from(self.remainder_us) + delta_us;
        let advance_ms = (total_us / 1_000).min(u64::from(u32::MAX)) as u32;
        self.remainder_us = (total_us % 1_000) as u32;
        self.now_ms += advance_ms;
    }

    fn update_budget(&mut self) {
        if self.unlimited {
            return;
        }

        let delta = self.heap.peek().map_or(0, |Reverse(entry)| {
            i64::from(self.now_ms) - i64::from(entry.deadline_ms)
        });

        let next_budget_us = delta + (delta / 100).max(-(MIN_BUDGET_US as i64));
        self.budget_us =
            next_budget_us.clamp(MIN_BUDGET_US as i64, self.max_budget_us as i64) as u64;
    }
}

impl Default for AmortizedExecutor {
    fn default() -> Self {
        Self {
            now_ms: 0,
            remainder_us: 0,
            budget_us: INITIAL_BUDGET_US,
            max_budget_us: MAX_BUDGET_US,
            unlimited: false,
            next_id: 0,
            tasks: HashMap::new(),
            heap: BinaryHeap::new(),
        }
    }
}

/// Weight of the newest sample in the effort EMA.
const EMA_NEW_WEIGHT: u64 = 2;
const EMA_TOTAL_WEIGHT: u64 = 5;
/// Effort used until the first real measurement arrives.
const INITIAL_EFFORT_US: u64 = 100;
const WARN_TASK_OVERRUN: u64 = 1000;
const WARN_TASK_EFFORT: u64 = 1000;
const MIN_BUDGET_US: u64 = 100;
const INITIAL_BUDGET_US: u64 = 100;
/// A single frame may not spend more than this on callbacks.
const MAX_BUDGET_US: u64 = 30_000;
/// Odd 32-bit constant for the deterministic registration jitter.
const JITTER_HASH: u32 = 0x9E37_79B9;

fn to_period_ms(period: Duration) -> u32 {
    period.as_millis().max(1).min(u128::from(u32::MAX)) as u32
}

fn jitter(id: u32, period_ms: u32) -> u32 {
    if period_ms <= 1 {
        return 0;
    }
    id.wrapping_mul(JITTER_HASH) % period_ms
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn count_calls(executor: &mut AmortizedExecutor) -> Arc<AtomicU64> {
        let count = Arc::new(AtomicU64::new(0));
        let callback_count = Arc::clone(&count);
        executor.add(Duration::from_millis(1), move |_| {
            callback_count.fetch_add(1, AtomicOrdering::Relaxed);
        });
        count
    }

    fn load(count: &AtomicU64) -> u64 {
        count.load(AtomicOrdering::Relaxed)
    }

    #[test]
    fn runs_at_first_deadline() {
        let mut executor = AmortizedExecutor::new_unlimited();
        let count = count_calls(&mut executor);
        executor.execute(999);
        assert_eq!(load(&count), 0);
        executor.execute(1);
        assert_eq!(load(&count), 1);
    }

    #[test]
    fn fixed_delay_counts_from_actual_run_time() {
        let mut executor = AmortizedExecutor::new_unlimited();
        let count = count_calls(&mut executor);
        executor.execute(1_000);
        assert_eq!(load(&count), 1);
        executor.execute(999);
        assert_eq!(load(&count), 1);
        executor.execute(1);
        assert_eq!(load(&count), 2);
    }

    #[test]
    fn removed_task_does_not_run() {
        let mut executor = AmortizedExecutor::new_unlimited();
        let count = Arc::new(AtomicU64::new(0));
        let callback_count = Arc::clone(&count);
        let handle = executor.add(Duration::from_millis(1), move |_| {
            callback_count.fetch_add(1, AtomicOrdering::Relaxed);
        });
        executor.remove(handle);
        executor.execute(10_000);
        assert_eq!(load(&count), 0);
    }

    #[test]
    fn unlimited_runs_every_due_task() {
        let mut executor = AmortizedExecutor::new_unlimited();
        let count = Arc::new(AtomicU64::new(0));
        for _ in 0..5 {
            let callback_count = Arc::clone(&count);
            executor.add(Duration::from_millis(1), move |_| {
                callback_count.fetch_add(1, AtomicOrdering::Relaxed);
            });
        }
        executor.execute(10_000);
        assert_eq!(load(&count), 5);
    }

    #[test]
    fn set_period_changes_reschedule_interval() {
        let mut executor = AmortizedExecutor::new_unlimited();
        let count = Arc::new(AtomicU64::new(0));
        let control = Arc::new(Mutex::new(None));
        let callback_count = Arc::clone(&count);
        let callback_control = Arc::clone(&control);
        executor.add(Duration::from_millis(10), move |ctrl| {
            *callback_control.lock().unwrap() = Some(Arc::clone(ctrl));
            callback_count.fetch_add(1, AtomicOrdering::Relaxed);
        });

        executor.execute(10_000);
        assert_eq!(load(&count), 1);

        let ctrl = control.lock().unwrap().clone().unwrap();
        ctrl.set_period(Duration::from_millis(1));

        executor.execute(10_000);
        assert_eq!(load(&count), 2);
        executor.execute(1_000);
        assert_eq!(load(&count), 3);
        executor.execute(1_000);
        assert_eq!(load(&count), 4);
    }

    #[test]
    fn jitter_spreads_first_deadlines() {
        let mut executor = AmortizedExecutor::new_unlimited();
        let period_ms = 1_000;
        let count = Arc::new(AtomicU64::new(0));
        for _ in 0..4 {
            let callback_count = Arc::clone(&count);
            executor.add(Duration::from_millis(u64::from(period_ms)), move |_| {
                callback_count.fetch_add(1, AtomicOrdering::Relaxed);
            });
        }
        executor.execute(u64::from(period_ms) * 1_000);
        let expected = (0..4).filter(|id| jitter(*id, period_ms) == 0).count() as u64;
        assert_eq!(load(&count), expected);
        assert!(expected < 4, "jitter should delay some first runs");
    }
}
