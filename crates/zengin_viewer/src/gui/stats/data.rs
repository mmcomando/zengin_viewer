use std::sync::PoisonError;

use bevy::log::warn;
use bevy::prelude::*;

use crate::zengin::{GameNpc, MenuDefinition, MenuItemDefinition, ScriptResource};

/// The projected `menu_status` screen, `None` when MENU.DAT could not be read.
#[derive(Resource, Default)]
pub(crate) struct StatsMenuData(pub(super) Option<MenuDefinition>);

impl StatsMenuData {
    /// The loaded status screen definition.
    pub(crate) fn definition(&self) -> Option<&MenuDefinition> {
        self.0.as_ref()
    }
}

/// Values of the character sheet shown by `menu_status`.
#[derive(Debug, Default)]
pub(crate) struct StatsSnapshot {
    guild: String,
    level: i32,
    exp: i32,
    exp_next: i32,
    lp: i32,
    magic_name: String,
    magic_circle: String,
    strength: i32,
    dexterity: i32,
    mana: i32,
    mana_max: i32,
    hitpoints: i32,
    hitpoints_max: i32,
}

impl StatsSnapshot {
    /// Reads the player stats and the menu text arrays they need. The script
    /// lock is taken once after the npc locks are released.
    pub(crate) fn from_npc(npc: &GameNpc, script: &ScriptResource) -> Self {
        let (
            guild,
            level,
            exp,
            exp_next,
            lp,
            strength,
            dexterity,
            mana,
            mana_max,
            hitpoints,
            hitpoints_max,
        ) = {
            let typed = npc
                .handle()
                .typed()
                .read()
                .unwrap_or_else(PoisonError::into_inner);
            (
                typed.guild,
                typed.level,
                typed.exp,
                typed.exp_next,
                typed.lp,
                typed.strength,
                typed.dexterity,
                typed.mana,
                typed.mana_max,
                typed.hitpoints,
                typed.hitpoints_max,
            )
        };
        let magic_circle = npc
            .handle()
            .runtime()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .talents
            .skill(TALENT_MAGE);

        let (guilds, talents, talent_skills) = {
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

        let guild = array_item(&guilds, guild).map_or_else(
            || {
                warn!("Guild({guild}) has no TXT_GUILDS entry");
                guild.to_string()
            },
            ToOwned::to_owned,
        );
        let magic_name = array_item(&talents, TALENT_MAGE)
            .unwrap_or_default()
            .to_owned();
        let magic_circle = array_item(&talent_skills, TALENT_MAGE).map_or_else(
            || magic_circle.to_string(),
            |encoded| str_enum(encoded, magic_circle),
        );

        Self {
            guild,
            level,
            exp,
            exp_next,
            lp,
            magic_name,
            magic_circle,
            strength,
            dexterity,
            mana,
            mana_max,
            hitpoints,
            hitpoints_max,
        }
    }

    pub(crate) fn guild(&self) -> &str {
        &self.guild
    }

    pub(crate) fn level(&self) -> i32 {
        self.level
    }

    /// Text of one menu widget: the snapshot value when the widget is in
    /// scope, the MENU.DAT text otherwise.
    pub(crate) fn text_for(&self, item: &MenuItemDefinition) -> String {
        match item.name.as_str() {
            "MENU_ITEM_PLAYERGUILD" => self.guild.clone(),
            "MENU_ITEM_LEVEL" => self.level.to_string(),
            "MENU_ITEM_TALENT_7_TITLE" => self.magic_name.clone(),
            "MENU_ITEM_TALENT_7_SKILL" => self.magic_circle.clone(),
            "MENU_ITEM_EXP" => self.exp.to_string(),
            "MENU_ITEM_LEVEL_NEXT" => self.exp_next.to_string(),
            "MENU_ITEM_LEARN" => self.lp.to_string(),
            "MENU_ITEM_ATTRIBUTE_1" => self.strength.to_string(),
            "MENU_ITEM_ATTRIBUTE_2" => self.dexterity.to_string(),
            "MENU_ITEM_ATTRIBUTE_3" => format!("{}/{}", self.mana, self.mana_max),
            "MENU_ITEM_ATTRIBUTE_4" => format!("{}/{}", self.hitpoints, self.hitpoints_max),
            _ => item.title().to_owned(),
        }
    }
}

/// Magic circle talent slot in G2.
const TALENT_MAGE: i32 = 7;

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
        StatsSnapshot {
            guild: "Paladyn".to_owned(),
            level: 12,
            exp: 3456,
            exp_next: 4000,
            lp: 7,
            magic_name: "Magia".to_owned(),
            magic_circle: "3".to_owned(),
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
    fn out_of_scope_widgets_keep_menu_text() {
        assert_eq!(
            snapshot().text_for(&item("MENU_ITEM_ARMOR_1_TITLE")),
            "menu text"
        );
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
