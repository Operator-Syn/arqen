#![allow(dead_code)]

use ratatui::style::Color;

// Terminal-dashboard palette: black canvas, cool borders, pastel-blue identity accent.
pub(crate) const BACKGROUND: Color = Color::Rgb(5, 8, 12); // #05080C
pub(crate) const SURFACE: Color = Color::Rgb(9, 14, 21); // #090E15
pub(crate) const SURFACE_RAISED: Color = Color::Rgb(18, 29, 43); // #121D2B
pub(crate) const SELECTION: Color = Color::Rgb(25, 78, 111); // reference selected-row blue
pub(crate) const PRIMARY: Color = Color::Rgb(157, 217, 247); // #9DD9F7
pub(crate) const PRIMARY_STRONG: Color = Color::Rgb(111, 190, 231); // #6FBEE7
pub(crate) const TEXT: Color = Color::Rgb(222, 235, 244); // #DEEBF4
pub(crate) const MUTED: Color = Color::Rgb(132, 149, 163); // #8495A3
pub(crate) const SUCCESS: Color = Color::Rgb(165, 224, 194); // #A5E0C2
pub(crate) const WARNING: Color = Color::Rgb(244, 210, 155); // #F4D29B
pub(crate) const DANGER: Color = Color::Rgb(243, 169, 184); // #F3A9B8
pub(crate) const BORDER: Color = Color::Rgb(74, 91, 108); // #4A5B6C

// Web surfaces use the same tokens as the terminal renderer.
pub(crate) const BACKGROUND_HEX: &str = "#05080C";
pub(crate) const SURFACE_HEX: &str = "#090E15";
pub(crate) const SURFACE_RAISED_HEX: &str = "#121D2B";
pub(crate) const PRIMARY_HEX: &str = "#9DD9F7";
pub(crate) const PRIMARY_STRONG_HEX: &str = "#6FBEE7";
pub(crate) const TEXT_HEX: &str = "#DEEBF4";
pub(crate) const MUTED_HEX: &str = "#8495A3";
pub(crate) const SUCCESS_HEX: &str = "#A5E0C2";
pub(crate) const DANGER_HEX: &str = "#F3A9B8";
pub(crate) const BORDER_HEX: &str = "#4A5B6C";
