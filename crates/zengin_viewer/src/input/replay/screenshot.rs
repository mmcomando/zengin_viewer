use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};

use super::failures::TestFailures;
use super::state::ActionReplay;

/// Size the screenshots are downscaled to before comparison. Coarse enough to
/// ignore anti-aliasing and GPU noise, fine enough to catch UI and lighting
/// changes.
const COMPARE_GRID: (u32, u32) = (64, 48);

/// Mean absolute channel difference (in `0..=1`) a grid cell needs to reach to
/// count as changed.
const COMPARE_CELL_THRESHOLD: f32 = 16.0 / 255.0;

/// Fraction of changed grid cells that is still accepted as the same image.
pub(super) const COMPARE_MAX_CHANGED_FRACTION: f32 = 0.02;

/// Schedules a screenshot save into the scenario workdir. A `test` screenshot
/// additionaly compares with (or blesses) the matching reference screenshot.
/// Returns `true` when the screenshot path is invalid and the replay must be
/// disabled.
#[allow(clippy::cognitive_complexity)]
pub(super) fn apply_replay_screenshot(
    replay: &ActionReplay,
    path: &String,
    commands: &mut Commands,
    test: bool,
) -> bool {
    if replay.renderless {
        if test {
            warn!(
                "Skipping TestScreenshot {path:?}: test screenshots need rendering, \
                 no comparison was done"
            );
        } else {
            warn!("Skipping Screenshot {path:?}: screenshots need rendering");
        }
        return false;
    }
    let Some(file_path) = resolve_scenario_path(&replay.workdir, path) else {
        warn!(
            "Replay disabled: screenshot path {path:?} must be relative to the \
             scenario workdir {:?}",
            replay.workdir
        );
        return true;
    };
    if let Some(parent) = file_path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        warn!("Failed to create screenshot dir {parent:?}: {error}");
    }
    let reference = test.then(|| replay.reference_root.join(path));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_scenario_screenshot(
            file_path,
            reference,
            replay.update_reference,
        ));
    false
}

/// Saves the captured screenshot into the scenario workdir and, in a test
/// scenario run, compares it with the reference screenshot or, with
/// `--update-reference`, blesses it. Failures are collected instead of
/// panicking, so the scenario keeps running and its logs are gathered before
/// the result is reported at exit.
fn save_scenario_screenshot(
    file_path: PathBuf,
    reference: Option<PathBuf>,
    update_reference: bool,
) -> impl FnMut(On<ScreenshotCaptured>, ResMut<TestFailures>) {
    move |captured: On<ScreenshotCaptured>, mut failures: ResMut<TestFailures>| {
        let _perf = dev_util::perf_debug::PerfDebug::new(
            "input::replay::screenshot::save_scenario_screenshot",
        );
        let image = match captured.image.clone().try_into_dynamic() {
            Ok(image) => image,
            Err(error) => {
                failures.push(format!(
                    "Failed to convert the screenshot for {} to an image: {error:?}",
                    file_path.display()
                ));
                return;
            }
        };
        if let Err(error) = image.save_with_format(&file_path, ImageFormat::Png) {
            failures.push(format!(
                "Failed to save screenshot {}: {error}",
                file_path.display()
            ));
            return;
        }
        let Some(reference) = &reference else {
            return;
        };
        if update_reference {
            if let Err(error) = bless_reference(&file_path, reference) {
                failures.push(error);
            }
            return;
        }
        if let Err(error) = compare_with_reference(&image, &file_path, reference) {
            failures.push(error);
        }
    }
}

/// Copies a produced screenshot over its reference screenshot.
fn bless_reference(file_path: &Path, reference: &Path) -> Result<(), String> {
    if let Some(parent) = reference.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Err(format!(
            "Failed to create reference dir {}: {error}",
            parent.display()
        ));
    }
    if let Err(error) = fs::copy(file_path, reference) {
        return Err(format!(
            "Failed to copy {} to reference {}: {error}",
            file_path.display(),
            reference.display()
        ));
    }
    info!("Updated reference screenshot {}", reference.display());
    Ok(())
}

/// Compares a produced screenshot with its reference. A match only logs the
/// difference so passing runs stay quiet; a mismatch is returned as a test
/// failure.
fn compare_with_reference(
    actual: &DynamicImage,
    actual_path: &Path,
    reference: &Path,
) -> Result<(), String> {
    let expected = match image::open(reference) {
        Ok(image) => image,
        Err(error) => {
            return Err(format!(
                "Reference screenshot {} is missing or unreadable ({error}). \
                 Run with --update-reference to create it",
                reference.display()
            ));
        }
    };
    let report = compare_images(actual, &expected)?;
    if report.changed_fraction <= COMPARE_MAX_CHANGED_FRACTION {
        info!(
            "Screenshot {} matches its reference (changed cells {:.2}%, mean diff {:.2}/255)",
            actual_path.display(),
            report.changed_fraction * 100.0,
            report.mean_diff * 255.0
        );
        return Ok(());
    }
    let diff_path = diff_path(actual_path);
    if let Err(error) = report
        .diff_image
        .save_with_format(&diff_path, ImageFormat::Png)
    {
        warn!(
            "Failed to save screenshot difference {}: {error}",
            diff_path.display()
        );
    }
    Err(format!(
        "Screenshot {} differs from the reference {}: changed cells {:.1}% (threshold {:.1}%), \
         mean diff {:.1}/255. See {}",
        actual_path.display(),
        reference.display(),
        report.changed_fraction * 100.0,
        COMPARE_MAX_CHANGED_FRACTION * 100.0,
        report.mean_diff * 255.0,
        diff_path.display()
    ))
}

/// Result of comparing two screenshots on the coarse comparison grid.
pub(super) struct CompareReport {
    /// Fraction of grid cells whose mean channel difference exceeded the cell
    /// threshold.
    pub(super) changed_fraction: f32,
    /// Mean absolute channel difference over all grid cells, `0..=1`.
    pub(super) mean_diff: f32,
    /// Full resolution absolute difference, for debugging a mismatch.
    diff_image: DynamicImage,
}

/// Compares two same-sized screenshots.
///
/// Both images are downscaled to a coarse grid before the difference is
/// computed, so anti-aliasing and minor GPU noise do not count as a change,
/// while UI, geometry and lighting changes do. A size mismatch is an error,
/// which is a large change by itself.
pub(super) fn compare_images(
    actual: &DynamicImage,
    expected: &DynamicImage,
) -> Result<CompareReport, String> {
    if actual.dimensions() != expected.dimensions() {
        return Err(format!(
            "Screenshot size {:?} does not match the reference size {:?}",
            actual.dimensions(),
            expected.dimensions()
        ));
    }
    let diff_image = difference_image(actual, expected);
    let (width, height) = COMPARE_GRID;
    let actual_grid = actual
        .resize_exact(width, height, FilterType::Triangle)
        .to_rgb8();
    let expected_grid = expected
        .resize_exact(width, height, FilterType::Triangle)
        .to_rgb8();
    let mut changed = 0u32;
    let mut diff_sum = 0.0f32;
    for (actual_pixel, expected_pixel) in actual_grid.pixels().zip(expected_grid.pixels()) {
        let cell_diff = actual_pixel
            .0
            .iter()
            .zip(expected_pixel.0.iter())
            .map(|(actual, expected)| f32::from(actual.abs_diff(*expected)))
            .sum::<f32>()
            / (3.0 * 255.0);
        diff_sum += cell_diff;
        if cell_diff > COMPARE_CELL_THRESHOLD {
            changed += 1;
        }
    }
    let cells = width.saturating_mul(height);
    Ok(CompareReport {
        changed_fraction: changed as f32 / cells as f32,
        mean_diff: diff_sum / cells as f32,
        diff_image,
    })
}

/// Builds a full resolution absolute difference image.
fn difference_image(actual: &DynamicImage, expected: &DynamicImage) -> DynamicImage {
    let actual = actual.to_rgb8();
    let expected = expected.to_rgb8();
    let mut diff = actual.clone();
    for (output, (actual, expected)) in diff
        .pixels_mut()
        .zip(actual.pixels().zip(expected.pixels()))
    {
        for (output, (actual, expected)) in output
            .0
            .iter_mut()
            .zip(actual.0.iter().zip(expected.0.iter()))
        {
            *output = actual.abs_diff(*expected);
        }
    }
    DynamicImage::ImageRgb8(diff)
}

/// Difference image path next to the workdir screenshot: `screen.png` becomes
/// `screen.diff.png`.
pub(super) fn diff_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().map_or_else(
        || "screenshot".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    path.with_file_name(format!("{stem}.diff.png"))
}

/// Resolves a relative path inside the scenario workdir.
///
/// Returns `None` for absolute paths, which abort the whole replay.
fn resolve_scenario_path(workdir: &Path, path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }
    Some(workdir.join(path))
}
