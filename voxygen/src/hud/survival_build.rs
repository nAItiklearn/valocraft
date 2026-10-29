//! Survival build mode panel: shows the selected block, how many of it are
//! left, the build controls and why the last edit was rejected.

use super::{
    CRITICAL_HP_COLOR, TEXT_BG, TEXT_COLOR, TEXT_COLOR_GREY, UI_HIGHLIGHT_0, item_imgs::ItemImgs,
    util,
};
use crate::{GlobalState, game_input::GameInput, ui::fonts::Fonts};
use common::comp::{
    item::{BuildFailedReason, BuildingBlock, Item, ItemI18n, item_key::ItemKey},
    tool::ToolKind,
};
use conrod_core::{
    Color, Colorable, Positionable, Sizeable, Widget, WidgetCommon,
    widget::{self, Image, Rectangle, Text},
    widget_ids,
};
use i18n::Localization;

/// What the HUD needs to know about survival build mode. Only present while
/// build mode is active.
pub struct SurvivalBuildInfo<'a> {
    pub block: Option<BuildingBlock>,
    /// An item of the selected block, for its name and icon.
    pub item: Option<&'a Item>,
    /// How many of the selected block are in the inventory.
    pub count: u32,
    /// Why the most recent edit was rejected, while it is still relevant.
    pub failure: Option<BuildFailedReason>,
}

widget_ids! {
    struct Ids {
        bg,
        accent,
        icon_frame,
        icon,
        title,
        name,
        count,
        controls,
        failure_bg,
        failure,
    }
}

const WIDTH: f64 = 320.0;
const HEIGHT: f64 = 62.0;
const ICON_SIZE: f64 = 40.0;
/// Sits right above the skillbar and its health/energy bars.
const BOTTOM_MARGIN: f64 = 168.0;
const BG_COLOR: Color = Color::Rgba(0.02, 0.03, 0.04, 0.62);
const ICON_BG_COLOR: Color = Color::Rgba(1.0, 1.0, 1.0, 0.06);

#[derive(WidgetCommon)]
pub struct SurvivalBuild<'a> {
    info: &'a SurvivalBuildInfo<'a>,
    global_state: &'a GlobalState,
    item_imgs: &'a ItemImgs,
    fonts: &'a Fonts,
    i18n: &'a Localization,
    item_i18n: &'a ItemI18n,
    #[conrod(common_builder)]
    common: widget::CommonBuilder,
}

impl<'a> SurvivalBuild<'a> {
    pub fn new(
        info: &'a SurvivalBuildInfo<'a>,
        global_state: &'a GlobalState,
        item_imgs: &'a ItemImgs,
        fonts: &'a Fonts,
        i18n: &'a Localization,
        item_i18n: &'a ItemI18n,
    ) -> Self {
        Self {
            info,
            global_state,
            item_imgs,
            fonts,
            i18n,
            item_i18n,
            common: widget::CommonBuilder::default(),
        }
    }

    fn key(&self, input: GameInput) -> String {
        self.global_state
            .settings
            .controls
            .get_binding(input)
            .map_or_else(|| "-".to_owned(), |key| key.display_shortest())
    }

    fn failure_text(&self, reason: BuildFailedReason) -> String {
        let key = match reason {
            BuildFailedReason::Protected => "hud-build-fail-protected",
            BuildFailedReason::OutOfReach => "hud-build-fail-out_of_reach",
            BuildFailedReason::RequiresTool(ToolKind::Pick) => "hud-build-fail-requires_pick",
            BuildFailedReason::RequiresTool(_) => "hud-build-fail-requires_tool",
            BuildFailedReason::Unbreakable => "hud-build-fail-unbreakable",
            BuildFailedReason::Obstructed => "hud-build-fail-obstructed",
            BuildFailedReason::MissingBlock => "hud-build-fail-missing_block",
        };
        self.i18n.get_msg(key).into_owned()
    }
}

pub struct State {
    ids: Ids,
}

impl Widget for SurvivalBuild<'_> {
    type Event = ();
    type State = State;
    type Style = ();

    fn init_state(&self, id_gen: widget::id::Generator) -> Self::State {
        State {
            ids: Ids::new(id_gen),
        }
    }

    fn style(&self) -> Self::Style {}

    fn update(self, args: widget::UpdateArgs<Self>) -> Self::Event {
        common_base::prof_span!("SurvivalBuild::update");
        let widget::UpdateArgs { state, ui, .. } = args;
        let ids = &state.ids;
        let cyri = &self.fonts.cyri;

        Rectangle::fill([WIDTH, HEIGHT])
            .color(BG_COLOR)
            .mid_bottom_with_margin_on(ui.window, BOTTOM_MARGIN)
            .set(ids.bg, ui);
        Rectangle::fill([WIDTH, 2.0])
            .color(UI_HIGHLIGHT_0)
            .mid_top_of(ids.bg)
            .set(ids.accent, ui);

        // Selected block
        Rectangle::fill([ICON_SIZE + 6.0, ICON_SIZE + 6.0])
            .color(ICON_BG_COLOR)
            .mid_left_with_margin_on(ids.bg, 8.0)
            .set(ids.icon_frame, ui);
        let item = self.info.item.filter(|_| self.info.block.is_some());
        if let Some(item) = item
            && let Some(icon) = self
                .item_imgs
                .img_ids_or_not_found_img(ItemKey::from(item))
                .first()
        {
            Image::new(*icon)
                .w_h(ICON_SIZE, ICON_SIZE)
                .middle_of(ids.icon_frame)
                .set(ids.icon, ui);
        }

        Text::new(&self.i18n.get_msg("hud-build-title"))
            .top_left_with_margins_on(ids.bg, 7.0, ICON_SIZE + 22.0)
            .font_id(cyri.conrod_id)
            .font_size(cyri.scale(11))
            .color(UI_HIGHLIGHT_0)
            .set(ids.title, ui);

        let (name, name_color) = match item {
            Some(item) => (
                util::item_text(item, self.i18n, self.item_i18n).0,
                if self.info.count > 0 {
                    TEXT_COLOR
                } else {
                    TEXT_COLOR_GREY
                },
            ),
            None => (
                self.i18n.get_msg("hud-build-no_block").into_owned(),
                TEXT_COLOR_GREY,
            ),
        };
        Text::new(&name)
            .down_from(ids.title, 3.0)
            .font_id(cyri.conrod_id)
            .font_size(cyri.scale(16))
            .color(name_color)
            .set(ids.name, ui);
        if item.is_some() {
            Text::new(&format!("x{}", self.info.count))
                .top_right_with_margins_on(ids.bg, 20.0, 12.0)
                .font_id(cyri.conrod_id)
                .font_size(cyri.scale(16))
                .color(if self.info.count > 0 {
                    TEXT_COLOR
                } else {
                    CRITICAL_HP_COLOR
                })
                .set(ids.count, ui);
        }

        let controls = if item.is_some() {
            self.i18n
                .get_msg_ctx("hud-build-controls", &i18n::fluent_args! {
                    "break" => self.key(GameInput::Primary),
                    "place" => self.key(GameInput::Secondary),
                    "pick" => self.key(GameInput::Roll),
                    "exit" => self.key(GameInput::ToggleBuildMode),
                })
                .into_owned()
        } else {
            self.i18n.get_msg("hud-build-gather_hint").into_owned()
        };
        Text::new(&controls)
            .bottom_left_with_margins_on(ids.bg, 7.0, ICON_SIZE + 22.0)
            .font_id(cyri.conrod_id)
            .font_size(cyri.scale(11))
            .color(TEXT_COLOR_GREY)
            .set(ids.controls, ui);

        // Why the last edit was rejected, right above the panel.
        if let Some(reason) = self.info.failure {
            let text = self.failure_text(reason);
            Text::new(&text)
                .mid_top_with_margin_on(ids.bg, -22.0)
                .font_id(cyri.conrod_id)
                .font_size(cyri.scale(15))
                .color(TEXT_BG)
                .set(ids.failure_bg, ui);
            Text::new(&text)
                .bottom_left_with_margins_on(ids.failure_bg, 1.0, 1.0)
                .font_id(cyri.conrod_id)
                .font_size(cyri.scale(15))
                .color(CRITICAL_HP_COLOR)
                .set(ids.failure, ui);
        }
    }
}
