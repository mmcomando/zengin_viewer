use std::sync::PoisonError;

use bevy::log::warn;
use bevy::prelude::*;

use crate::{GameNpc, MenuDefinition, MenuItemDefinition, ScriptResource};

/// The projected `menu_status` screen, `None` when MENU.DAT could not be read.
#[derive(Resource, Default)]
pub struct StatsMenuData(pub Option<MenuDefinition>);

impl StatsMenuData {
    /// The loaded status screen definition.
    pub fn definition(&self) -> Option<&MenuDefinition> {
        self.0.as_ref()
    }
}

/// One row of the `UMIEJĘTNOŚCI` section.
#[derive(Debug, Default, Clone)]
struct TalentRow {
    title: String,
    skill: String,
    value: i32,
}

/// Values of the character sheet shown by `menu_status`.
#[derive(Debug, Default)]
pub struct StatsSnapshot {
    guild: String,
    level: i32,
    exp: i32,
    exp_next: i32,
    lp: i32,
    talents: Vec<TalentRow>,
    protection: [i32; PROTECTION_COUNT],
    strength: i32,
    dexterity: i32,
    mana: i32,
    mana_max: i32,
    hitpoints: i32,
    hitpoints_max: i32,
}

impl StatsSnapshot {
    /// Reads the player stats and the menu text arrays they need. The script
    /// lock is released before the npc data is read.
    pub fn from_npc(npc: &GameNpc, script: &ScriptResource) -> Self {
        let (guilds, talent_titles, talent_skills) = {
            let mut script = script.lock();
            (
                script.string_array("txt_guilds").unwrap_or_default(),
                script.string_array("txt_talents").unwrap_or_default(),
                script
                    .string_array("txt_talents_skills")
                    .unwrap_or_default(),
            )
        };
        if guilds.is_empty() {
            warn!("TXT_GUILDS is unavailable, the guild shows the raw id");
        }
        if talent_titles.is_empty() {
            warn!("TXT_TALENTS is unavailable, talent titles stay empty");
        }

        let typed = npc
            .handle()
            .typed()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let runtime = npc
            .handle()
            .runtime()
            .read()
            .unwrap_or_else(PoisonError::into_inner);

        let guild = array_item(&guilds, typed.guild).map_or_else(
            || {
                warn!("Guild({}) has no TXT_GUILDS entry", typed.guild);
                typed.guild.to_string()
            },
            ToOwned::to_owned,
        );
        let talents = (0..TALENT_COUNT)
            .map(|talent| {
                let skill_value = runtime.talents.skill(talent as i32);
                let title = array_item(&talent_titles, talent as i32)
                    .unwrap_or_default()
                    .to_owned();
                let skill = array_item(&talent_skills, talent as i32)
                    .map_or_else(String::new, |encoded| str_enum(encoded, skill_value));
                let value = typed.hitchance.get(talent).copied().unwrap_or_default();
                TalentRow {
                    title,
                    skill,
                    value,
                }
            })
            .collect();

        Self {
            guild,
            level: typed.level,
            exp: typed.exp,
            exp_next: typed.exp_next,
            lp: typed.lp,
            talents,
            protection: typed.protection,
            strength: typed.strength,
            dexterity: typed.dexterity,
            mana: typed.mana,
            mana_max: typed.mana_max,
            hitpoints: typed.hitpoints,
            hitpoints_max: typed.hitpoints_max,
        }
    }

    pub fn guild(&self) -> &str {
        &self.guild
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    /// Text of one menu widget: the snapshot value when the widget is in
    /// scope, the MENU.DAT text otherwise.
    pub fn text_for(&self, item: &MenuItemDefinition) -> String {
        if let Some((talent, field)) = talent_widget(&item.name) {
            if let Some(row) = self.talents.get(talent) {
                return match field {
                    TalentField::Title => row.title.clone(),
                    TalentField::Skill => row.skill.clone(),
                    TalentField::Value => format!("{}%", row.value),
                };
            }
            return item.title().to_owned();
        }
        match item.name.as_str() {
            "MENU_ITEM_PLAYERGUILD" => self.guild.clone(),
            "MENU_ITEM_LEVEL" => self.level.to_string(),
            "MENU_ITEM_EXP" => self.exp.to_string(),
            "MENU_ITEM_LEVEL_NEXT" => self.exp_next.to_string(),
            "MENU_ITEM_LEARN" => self.lp.to_string(),
            "MENU_ITEM_ATTRIBUTE_1" => self.strength.to_string(),
            "MENU_ITEM_ATTRIBUTE_2" => self.dexterity.to_string(),
            "MENU_ITEM_ATTRIBUTE_3" => format!("{}/{}", self.mana, self.mana_max),
            "MENU_ITEM_ATTRIBUTE_4" => format!("{}/{}", self.hitpoints, self.hitpoints_max),
            "MENU_ITEM_ARMOR_1" => self.protection_value(PROT_BLUNT).to_string(),
            "MENU_ITEM_ARMOR_2" => self.protection_value(PROT_POINT).to_string(),
            "MENU_ITEM_ARMOR_3" => self.protection_value(PROT_FIRE).to_string(),
            "MENU_ITEM_ARMOR_4" => self.protection_value(PROT_MAGIC).to_string(),
            _ => item.title().to_owned(),
        }
    }

    fn protection_value(&self, index: usize) -> i32 {
        self.protection.get(index).copied().unwrap_or_default()
    }
}

/// Talent slots in G2 (`TALENT_MAX_G2`).
const TALENT_COUNT: usize = 22;

/// `C_Npc.protection` indices used by the `OCHRONA` rows.
const PROTECTION_COUNT: usize = 8;
const PROT_BLUNT: usize = 1;
const PROT_POINT: usize = 6;
const PROT_FIRE: usize = 3;
const PROT_MAGIC: usize = 5;

enum TalentField {
    Title,
    Skill,
    Value,
}

/// Resolves `MENU_ITEM_TALENT_<n>[_TITLE|_SKILL]` to its row and field. The
/// `_CIRCLE` widget is menu text, not a talent index.
fn talent_widget(name: &str) -> Option<(usize, TalentField)> {
    let rest = name.strip_prefix("MENU_ITEM_TALENT_")?;
    let (index, field) = if let Some(index) = rest.strip_suffix("_TITLE") {
        (index, TalentField::Title)
    } else if let Some(index) = rest.strip_suffix("_SKILL") {
        (index, TalentField::Skill)
    } else {
        (rest, TalentField::Value)
    };
    Some((index.parse().ok()?, field))
}

fn array_item(array: &[String], index: i32) -> Option<&str> {
    let index = usize::try_from(index).ok()?;
    array
        .get(index)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

/// Picks the `index` token of a `|`-separated menu string, skipping a `#`
/// prefixed lead token. Falls back to the first token when the index is out of
/// range, matching the original menu renderer.
fn str_enum(encoded: &str, index: i32) -> String {
    let body = encoded.split_once('#').map_or(encoded, |(_, rest)| rest);
    let first = body.split('|').next().unwrap_or_default();
    usize::try_from(index)
        .ok()
        .and_then(|index| body.split('|').nth(index))
        .unwrap_or(first)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> MenuItemDefinition {
        MenuItemDefinition {
            name: name.to_owned(),
            text: vec!["menu text".to_owned()],
            ..Default::default()
        }
    }

    fn snapshot() -> StatsSnapshot {
        let mut talents = vec![TalentRow::default(); TALENT_COUNT];
        talents[1] = TalentRow {
            title: "Br. jednor\u{119}czna".to_owned(),
            skill: "Mistrz".to_owned(),
            value: 67,
        };
        talents[7] = TalentRow {
            title: "Magia".to_owned(),
            skill: "3".to_owned(),
            value: 0,
        };
        StatsSnapshot {
            guild: "Paladyn".to_owned(),
            level: 12,
            exp: 3456,
            exp_next: 4000,
            lp: 7,
            talents,
            protection: [0, 158, 10, 108, 0, 58, 158, 0],
            strength: 40,
            dexterity: 25,
            mana: 30,
            mana_max: 50,
            hitpoints: 120,
            hitpoints_max: 150,
        }
    }

    #[test]
    fn scoped_widgets_use_snapshot_values() {
        let snapshot = snapshot();
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_PLAYERGUILD")), "Paladyn");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_ATTRIBUTE_3")), "30/50");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_ATTRIBUTE_4")), "120/150");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_LEARN")), "7");
    }

    #[test]
    fn talent_widgets_use_the_row_values() {
        let snapshot = snapshot();
        assert_eq!(
            snapshot.text_for(&item("MENU_ITEM_TALENT_1_TITLE")),
            "Br. jednor\u{119}czna"
        );
        assert_eq!(
            snapshot.text_for(&item("MENU_ITEM_TALENT_1_SKILL")),
            "Mistrz"
        );
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_TALENT_1")), "67%");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_TALENT_7_SKILL")), "3");
        assert_eq!(
            snapshot.text_for(&item("MENU_ITEM_TALENT_7_CIRCLE")),
            "menu text"
        );
    }

    #[test]
    fn out_of_scope_widgets_keep_menu_text() {
        assert_eq!(
            snapshot().text_for(&item("MENU_ITEM_ARMOR_1_TITLE")),
            "menu text"
        );
    }

    #[test]
    fn protection_widgets_use_the_matching_protection_rows() {
        let snapshot = snapshot();
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_ARMOR_1")), "158");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_ARMOR_2")), "158");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_ARMOR_3")), "108");
        assert_eq!(snapshot.text_for(&item("MENU_ITEM_ARMOR_4")), "58");
    }

    #[test]
    fn str_enum_picks_the_skill_token() {
        assert_eq!(str_enum("0|1|2|3", 2), "2");
        assert_eq!(str_enum("0|1|2|3", 9), "0");
        assert_eq!(str_enum("Zielony|Wojownik|Mistrz", -1), "Zielony");
        assert_eq!(str_enum("#lead|a|b", 1), "a");
    }

    #[test]
    fn array_item_rejects_negative_and_empty_entries() {
        let array = vec![String::new(), "value".to_owned()];
        assert_eq!(array_item(&array, -1), None);
        assert_eq!(array_item(&array, 0), None);
        assert_eq!(array_item(&array, 1), Some("value"));
        assert_eq!(array_item(&array, 2), None);
    }
}
