//! Emberwatch contract board and field journal.
//!
//! This is the first playable slice of Valocraft's frontier contract loop. The
//! journal is client-driven for now; rewards are paid through the existing
//! server chat-command pipeline so they still land in the authoritative
//! inventory.

use super::{Imgs, TEXT_COLOR, UI_HIGHLIGHT_0, UI_MAIN, map};
use crate::{GlobalState, profile::ContractProgress, ui::fonts::Fonts};
use client::Client;
use common::{
    character::CharacterId,
    comp::{self, Alignment, Content},
    map::{Marker, MarkerKind},
    terrain::CoordinateConversions,
    uid::{IdMaps, Uid},
};
use conrod_core::{
    Color, Colorable, Labelable, Positionable, Sizeable, Widget, WidgetCommon,
    widget::{self, Button, Image, Rectangle, Text},
    widget_ids,
};
use i18n::Localization;
use rand::RngExt;
use specs::{Join, WorldExt};
use vek::Vec2;

const BOARD_NAME: &str = "Hearthpost Ember Board";
const FACTION_NAME: &str = "Emberwatch";
const JOURNAL_TITLE: &str = "FIELD JOURNAL";
const BOARD_RANGE: f32 = 150.0;
const SITE_RANGE: f32 = 14.0;

const COLOR_EMBER: Color = Color::Rgba(0.95, 0.56, 0.18, 1.0);
const COLOR_SCOUT: Color = Color::Rgba(0.37, 0.79, 0.74, 1.0);
const COLOR_BOUNTY: Color = Color::Rgba(0.88, 0.32, 0.24, 1.0);
const COLOR_PANEL: Color = Color::Rgba(0.055, 0.075, 0.08, 0.97);
const COLOR_CARD: Color = Color::Rgba(0.09, 0.13, 0.135, 1.0);
const COLOR_MUTED: Color = Color::Rgba(0.56, 0.63, 0.62, 1.0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContractKind {
    Survey,
    Bounty,
    Courier,
}

impl ContractKind {
    fn label(self) -> &'static str {
        match self {
            Self::Survey => "SURVEY",
            Self::Bounty => "BOUNTY",
            Self::Courier => "COURIER",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Survey => COLOR_SCOUT,
            Self::Bounty => COLOR_BOUNTY,
            Self::Courier => COLOR_EMBER,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Survey => "Chart the Ashline",
            Self::Bounty => "A Threat in the Wilds",
            Self::Courier => "The Long Ember",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Survey => {
                "Record three frontier landmarks for the Emberwatch so the next caravan can travel \
                 safely."
            },
            Self::Bounty => {
                "Track a dangerous creature away from the road and put it down before it reaches \
                 the settlement."
            },
            Self::Courier => {
                "Carry a sealed dispatch to the frontier relay, then return to Hearthpost with the \
                 reply."
            },
        }
    }

    fn reward(self) -> u32 {
        match self {
            Self::Survey => 45,
            Self::Bounty => 70,
            Self::Courier => 55,
        }
    }

    fn reputation(self) -> u32 {
        match self {
            Self::Survey => 15,
            Self::Bounty => 25,
            Self::Courier => 20,
        }
    }
}

#[derive(Clone)]
struct BountyTarget {
    uid: Uid,
    name: Content,
    last_known: Vec2<f32>,
}

enum OfferData {
    Survey {
        points: [Vec2<f32>; 3],
    },
    Bounty {
        target: Option<BountyTarget>,
        last_known: Vec2<f32>,
    },
    Courier {
        destination: Vec2<f32>,
    },
}

struct Offer {
    kind: ContractKind,
    data: OfferData,
    hint: Vec2<f32>,
}

enum Objective {
    Survey {
        points: [Vec2<f32>; 3],
        visited: [bool; 3],
    },
    Bounty {
        target: Option<BountyTarget>,
        last_known: Vec2<f32>,
        missing_for: f32,
    },
    Courier {
        destination: Vec2<f32>,
        returning: bool,
    },
}

struct ActiveContract {
    kind: ContractKind,
    objective: Objective,
    reward: u32,
    reputation: u32,
}

pub struct State {
    progress: ContractProgress,
    board: Option<Vec2<f32>>,
    offers: Vec<Offer>,
    active: Option<ActiveContract>,
    server: String,
    character_id: Option<CharacterId>,
    dirty: bool,
    camp_spawned: bool,
}

pub enum Action {
    GrantCoins(u32),
    SpawnCamp,
    Message(String),
}

impl State {
    pub fn new(
        progress: ContractProgress,
        server: String,
        character_id: Option<CharacterId>,
    ) -> Self {
        let board = progress.board_anchor.map(|[x, y]| Vec2::new(x, y));
        Self {
            progress,
            board,
            offers: Vec::new(),
            active: None,
            server,
            character_id,
            dirty: false,
            camp_spawned: false,
        }
    }

    pub fn take_dirty(&mut self) -> bool { std::mem::take(&mut self.dirty) }

    pub fn save(&self, global_state: &mut GlobalState) {
        global_state.profile.set_contract_progress(
            &self.server,
            self.character_id,
            self.progress.clone(),
        );
        global_state
            .profile
            .save_to_file_warn(&global_state.config_dir);
    }

    pub fn progress(&self) -> &ContractProgress { &self.progress }

    pub fn board(&self) -> Option<Vec2<f32>> { self.board }

    pub fn near_board(&self, player_pos: Vec2<f32>) -> bool {
        self.board
            .is_some_and(|board| board.distance(player_pos) <= BOARD_RANGE)
    }

    fn offers(&self) -> &[Offer] { &self.offers }

    fn active(&self) -> Option<&ActiveContract> { self.active.as_ref() }

    pub fn beacon_ready(&self, player_pos: Vec2<f32>) -> bool {
        !self.progress.world_change && self.progress.completed >= 3 && self.near_board(player_pos)
    }

    /// Advance objectives once per frame and return any effects the HUD needs
    /// to apply to the authoritative game state.
    pub fn update(&mut self, client: &Client, dt: f32) -> Vec<Action> {
        let Some(player_pos) = client.position() else {
            return Vec::new();
        };
        let player_xy = player_pos.xy();
        let mut actions = Vec::new();

        if self.board.is_none() {
            self.board = Some(self.find_board(client, player_xy));
            self.progress.board_anchor = self.board.map(|b| [b.x, b.y]);
            self.dirty = true;
        }
        let board = self.board.expect("board was just initialized");

        if self.active.is_none() && self.offers.is_empty() && self.near_board(player_xy) {
            self.offers = generate_offers(client, board);
        }

        let mut complete = false;
        let mut abandon = false;
        if let Some(active) = self.active.as_mut() {
            match &mut active.objective {
                Objective::Survey { points, visited } => {
                    if let Some(next) = visited
                        .iter()
                        .enumerate()
                        .find_map(|(i, seen)| (!*seen).then_some(i))
                        && points[next].distance(player_xy) <= SITE_RANGE
                    {
                        visited[next] = true;
                        let charted = visited.iter().filter(|seen| **seen).count();
                        actions.push(Action::Message(format!(
                            "Survey site charted ({charted}/3)."
                        )));
                        complete = visited.iter().all(|seen| *seen);
                    }
                },
                Objective::Bounty {
                    target,
                    last_known,
                    missing_for,
                } => {
                    let ecs = client.state().ecs();
                    let uids = ecs.read_resource::<IdMaps>();
                    let healths = ecs.read_storage::<comp::Health>();
                    let positions = ecs.read_storage::<comp::Pos>();
                    let target_entity = target.as_ref().and_then(|t| uids.uid_entity(t.uid));

                    if let Some(target) = target {
                        if let Some(entity) = target_entity {
                            if let Some(pos) = positions.get(entity) {
                                target.last_known = pos.0.xy();
                                *last_known = target.last_known;
                            }
                            if healths.get(entity).is_some_and(|h| h.is_dead) {
                                complete = true;
                            } else {
                                *missing_for = 0.0;
                            }
                        } else {
                            *missing_for += dt;
                            if *missing_for > 2.0 && target.last_known.distance(player_xy) <= 250.0
                            {
                                complete = true;
                            } else if *missing_for > 20.0 {
                                actions.push(Action::Message(
                                    "The trail has gone cold. Return to the Ember Board to pick \
                                     up the bounty again."
                                        .to_string(),
                                ));
                                abandon = true;
                            }
                        }
                    } else if last_known.distance(player_xy) <= SITE_RANGE {
                        complete = true;
                    }
                },
                Objective::Courier {
                    destination,
                    returning,
                } => {
                    if !*returning && destination.distance(player_xy) <= SITE_RANGE {
                        *returning = true;
                        actions.push(Action::Message(
                            "Dispatch delivered. Return to Hearthpost to report in.".to_string(),
                        ));
                    } else if *returning && board.distance(player_xy) <= BOARD_RANGE {
                        complete = true;
                    }
                },
            }
        }
        if complete {
            actions.extend(self.complete_active());
        } else if abandon {
            self.active = None;
            self.offers.clear();
        }

        if self.progress.world_change && !self.camp_spawned && self.near_board(player_xy) {
            self.camp_spawned = true;
            actions.push(Action::SpawnCamp);
        }

        actions
    }

    pub fn accept(&mut self, client: &Client, index: usize) -> Vec<Action> {
        let Some(player_pos) = client.position() else {
            return Vec::new();
        };
        if !self.near_board(player_pos.xy()) {
            return vec![Action::Message(format!(
                "The Ember Board is at {BOARD_NAME}. Return there before accepting a contract."
            ))];
        }
        if self.active.is_some() {
            return vec![Action::Message(
                "Finish your current contract before taking another.".to_string(),
            )];
        }
        let Some(offer) = self.offers.get(index) else {
            return Vec::new();
        };

        let (objective, reward, reputation) = match &offer.data {
            OfferData::Survey { points } => (
                Objective::Survey {
                    points: *points,
                    visited: [false; 3],
                },
                offer.kind.reward(),
                offer.kind.reputation(),
            ),
            OfferData::Bounty { target, last_known } => (
                Objective::Bounty {
                    target: target.clone(),
                    last_known: *last_known,
                    missing_for: 0.0,
                },
                offer.kind.reward(),
                offer.kind.reputation(),
            ),
            OfferData::Courier { destination } => (
                Objective::Courier {
                    destination: *destination,
                    returning: false,
                },
                offer.kind.reward(),
                offer.kind.reputation(),
            ),
        };
        let kind = offer.kind;
        self.offers.clear();
        self.active = Some(ActiveContract {
            kind,
            objective,
            reward,
            reputation,
        });
        vec![Action::Message(format!(
            "Contract accepted: {}. Open the map to find your objective.",
            kind.title()
        ))]
    }

    pub fn abandon(&mut self) -> Vec<Action> {
        if self.active.take().is_some() {
            self.offers.clear();
            vec![Action::Message(
                "Contract abandoned. The Ember Board will have new work when you return."
                    .to_string(),
            )]
        } else {
            Vec::new()
        }
    }

    pub fn found_beacon(&mut self, client: &Client) -> Vec<Action> {
        let Some(player_pos) = client.position() else {
            return Vec::new();
        };
        if self.progress.world_change
            || self.progress.completed < 3
            || !self.near_board(player_pos.xy())
        {
            return Vec::new();
        }
        self.progress.world_change = true;
        self.progress.reputation += 40;
        self.dirty = true;
        self.camp_spawned = true;
        vec![
            Action::Message(
                "The Emberwatch Beacon is lit. Hearthpost is now a permanent waystation on the \
                 frontier."
                    .to_string(),
            ),
            Action::GrantCoins(150),
            Action::SpawnCamp,
        ]
    }

    fn complete_active(&mut self) -> Vec<Action> {
        let Some(active) = self.active.take() else {
            return Vec::new();
        };
        self.progress.completed += 1;
        self.progress.reputation += active.reputation;
        self.dirty = true;
        self.offers.clear();

        let mut actions = vec![
            Action::Message(format!(
                "{} complete. +{} coins, +{} Emberwatch standing.",
                active.kind.title(),
                active.reward,
                active.reputation
            )),
            Action::GrantCoins(active.reward),
        ];
        if self.progress.completed >= 3 && !self.progress.world_change {
            actions.push(Action::Message(
                "Hearthpost is ready for its Beacon. Return to the Ember Board and light it."
                    .to_string(),
            ));
        }
        actions
    }

    pub fn markers(&self, player_pos: Vec2<f32>) -> Vec<map::ExtraMarker> {
        let mut markers = Vec::new();
        if let Some(board) = self.board {
            markers.push(extra_marker(
                player_pos,
                board,
                1,
                MarkerKind::Town,
                "Ember Board",
            ));
        }
        if let Some(active) = &self.active {
            let (pos, label, id, kind) = match &active.objective {
                Objective::Survey { points, visited } => {
                    let next = visited
                        .iter()
                        .enumerate()
                        .find_map(|(i, seen)| (!*seen).then_some(i))
                        .unwrap_or(0);
                    (
                        points[next],
                        "Survey Site",
                        100 + next as u64,
                        MarkerKind::Unknown,
                    )
                },
                Objective::Bounty { last_known, .. } => (
                    last_known.to_owned(),
                    "Bounty Trail",
                    200,
                    MarkerKind::Unknown,
                ),
                Objective::Courier {
                    destination,
                    returning,
                } => (
                    if *returning {
                        self.board.unwrap_or(*destination)
                    } else {
                        *destination
                    },
                    if *returning {
                        "Report to Hearthpost"
                    } else {
                        "Frontier Relay"
                    },
                    300,
                    MarkerKind::Unknown,
                ),
            };
            markers.push(extra_marker(player_pos, pos, id, kind, label));
        }
        if self.progress.world_change
            && let Some(board) = self.board
        {
            markers.push(extra_marker(
                player_pos,
                board,
                400,
                MarkerKind::Unknown,
                "Emberwatch Beacon",
            ));
        }
        markers
    }

    pub fn hud_tracker(&self, player_pos: Vec2<f32>) -> Option<(String, Color)> {
        if self.beacon_ready(player_pos) {
            return Some((
                "[U] EMBERWATCH BEACON READY — Open Field Journal to light the beacon".to_string(),
                COLOR_EMBER,
            ));
        }
        if let Some(active) = &self.active {
            return Some((
                format!(
                    "[U] {}  •  {}",
                    active.kind.title(),
                    active_progress(active, player_pos, self.board)
                ),
                active.kind.color(),
            ));
        }
        if self.near_board(player_pos) {
            return Some((
                format!(
                    "[U] Field Journal  •  {} ({} completed)",
                    BOARD_NAME, self.progress.completed
                ),
                COLOR_MUTED,
            ));
        }
        None
    }

    fn find_board(&self, client: &Client, player_xy: Vec2<f32>) -> Vec2<f32> {
        client
            .sites()
            .values()
            .filter(|site| site.marker.kind == MarkerKind::Town)
            .filter_map(|site| {
                let distance = site.marker.wpos.distance(player_xy);
                (distance <= 1500.0).then_some((distance, site.marker.wpos))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, pos)| pos)
            .unwrap_or(player_xy)
    }
}

fn extra_marker(
    player_pos: Vec2<f32>,
    pos: Vec2<f32>,
    id: u64,
    kind: MarkerKind,
    label: &str,
) -> map::ExtraMarker {
    map::ExtraMarker {
        recv_pos: player_pos,
        marker: Marker::at(pos)
            .with_id(("emberwatch-contract", id))
            .with_kind(kind)
            .with_label(Some(Content::Plain(label.to_string())))
            .with_quest_flag(true),
    }
}

fn generate_offers(client: &Client, board: Vec2<f32>) -> Vec<Offer> {
    let points = [
        pick_land_point(client, board, 120.0, 280.0),
        pick_land_point(client, board, 160.0, 320.0),
        pick_land_point(client, board, 120.0, 280.0),
    ];
    let (bounty_target, bounty_hint) = find_bounty(client, board);
    let courier_destination = pick_land_point(client, board, 180.0, 360.0);

    vec![
        Offer {
            kind: ContractKind::Survey,
            data: OfferData::Survey { points },
            hint: points[0],
        },
        Offer {
            kind: ContractKind::Bounty,
            data: OfferData::Bounty {
                target: bounty_target,
                last_known: bounty_hint,
            },
            hint: bounty_hint,
        },
        Offer {
            kind: ContractKind::Courier,
            data: OfferData::Courier {
                destination: courier_destination,
            },
            hint: courier_destination,
        },
    ]
}

fn find_bounty(client: &Client, board: Vec2<f32>) -> (Option<BountyTarget>, Vec2<f32>) {
    let ecs = client.state().ecs();
    let positions = ecs.read_storage::<comp::Pos>();
    let healths = ecs.read_storage::<comp::Health>();
    let alignments = ecs.read_storage::<comp::Alignment>();
    let stats = ecs.read_storage::<comp::Stats>();
    let uids = ecs.read_storage::<Uid>();

    let mut best: Option<(u8, f32, BountyTarget)> = None;
    for (uid, pos, health, alignment, stats) in
        (&uids, &positions, &healths, &alignments, &stats).join()
    {
        if health.is_dead {
            continue;
        }
        let priority = match alignment {
            Alignment::Enemy => 0,
            Alignment::Wild => 1,
            _ => continue,
        };
        let distance = pos.0.xy().distance(board);
        if distance > 700.0 {
            continue;
        }
        let candidate = BountyTarget {
            uid: *uid,
            name: stats.name.clone(),
            last_known: pos.0.xy(),
        };
        if best
            .as_ref()
            .is_none_or(|(best_priority, best_distance, _)| {
                priority < *best_priority
                    || (priority == *best_priority && distance < *best_distance)
            })
        {
            best = Some((priority, distance, candidate));
        }
    }

    if let Some((_, _, target)) = best {
        let hint = target.last_known;
        (Some(target), hint)
    } else {
        (None, pick_land_point(client, board, 140.0, 300.0))
    }
}

fn pick_land_point(client: &Client, center: Vec2<f32>, min: f32, max: f32) -> Vec2<f32> {
    let mut rng = rand::rng();
    let mut fallback = center + Vec2::new(max, 0.0);
    for _ in 0..16 {
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let distance = rng.random_range(min..max);
        let point = center + Vec2::new(angle.cos(), angle.sin()) * distance;
        fallback = point;
        let cpos = point.map(|e| e as i32).wpos_to_cpos();
        if client
            .world_data()
            .alt_at(cpos)
            .is_some_and(|alt| alt > 1.0)
        {
            return point;
        }
    }
    fallback
}

#[derive(Clone, Copy)]
pub enum Event {
    Close,
    Accept(usize),
    Abandon,
    FoundBeacon,
}

#[derive(WidgetCommon)]
pub struct View<'a> {
    state: &'a State,
    imgs: &'a Imgs,
    fonts: &'a Fonts,
    i18n: &'a Localization,
    player_pos: Vec2<f32>,
    #[conrod(common_builder)]
    common: widget::CommonBuilder,
}

pub struct ViewState {
    ids: Ids,
}

widget_ids! {
    struct Ids {
        bg,
        frame,
        close,
        title,
        subtitle,
        standing_bg,
        standing_fill,
        standing_bar,
        standing_text,
        board_text,
        section_title,
        cards[],
        card_accents[],
        card_kinds[],
        card_titles[],
        card_descriptions[],
        card_meta[],
        card_buttons[],
        active_card,
        active_accent,
        active_kind,
        active_title,
        active_description,
        active_progress,
        active_meta,
        abandon_button,
        empty_text,
        beacon_button,
        beacon_text,
        footer,
        key_hint,
    }
}

impl<'a> View<'a> {
    pub fn new(
        state: &'a State,
        imgs: &'a Imgs,
        fonts: &'a Fonts,
        i18n: &'a Localization,
        player_pos: Vec2<f32>,
    ) -> Self {
        Self {
            state,
            imgs,
            fonts,
            i18n,
            player_pos,
            common: widget::CommonBuilder::default(),
        }
    }

    fn button<'b>(
        &'b self,
        label: &'b str,
        parent: widget::Id,
        id: widget::Id,
        y: f64,
        ui: &mut conrod_core::UiCell,
    ) -> bool {
        Button::image(self.imgs.button)
            .w_h(230.0, 38.0)
            .mid_bottom_with_margin_on(parent, y)
            .hover_image(self.imgs.button_hover)
            .press_image(self.imgs.button_press)
            .label(label)
            .label_color(TEXT_COLOR)
            .label_font_size(self.fonts.cyri.scale(15))
            .label_font_id(self.fonts.cyri.conrod_id)
            .set(id, ui)
            .was_clicked()
    }

    fn rank(&self) -> &'static str {
        match self.state.progress.reputation {
            0..=24 => "Unproven",
            25..=59 => "Ember Scout",
            60..=99 => "Beacon Warden",
            _ => "Flamekeeper",
        }
    }
}

impl Widget for View<'_> {
    type Event = Vec<Event>;
    type State = ViewState;
    type Style = ();

    fn init_state(&self, id_gen: widget::id::Generator) -> Self::State {
        ViewState {
            ids: Ids::new(id_gen),
        }
    }

    fn style(&self) -> Self::Style {}

    fn update(self, args: widget::UpdateArgs<Self>) -> Self::Event {
        let widget::UpdateArgs { state, ui, .. } = args;
        let mut events = Vec::new();
        let progress = self.state.progress();
        let near_board = self.state.near_board(self.player_pos);
        let board = self.state.board();

        if state.ids.cards.len() < 3 {
            state.update(|state| {
                let mut id_gen = ui.widget_id_generator();
                state.ids.cards.resize(3, &mut id_gen);
                state.ids.card_accents.resize(3, &mut id_gen);
                state.ids.card_kinds.resize(3, &mut id_gen);
                state.ids.card_titles.resize(3, &mut id_gen);
                state.ids.card_descriptions.resize(3, &mut id_gen);
                state.ids.card_meta.resize(3, &mut id_gen);
                state.ids.card_buttons.resize(3, &mut id_gen);
            });
        }

        Image::new(self.imgs.diary_bg)
            .w_h(940.0, 620.0)
            .middle_of(ui.window)
            .color(Some(UI_MAIN))
            .set(state.ids.bg, ui);
        Image::new(self.imgs.diary_frame)
            .w_h(940.0, 620.0)
            .middle_of(state.ids.bg)
            .color(Some(UI_HIGHLIGHT_0))
            .set(state.ids.frame, ui);

        if Button::image(self.imgs.close_btn)
            .w_h(24.0, 25.0)
            .hover_image(self.imgs.close_btn_hover)
            .press_image(self.imgs.close_btn_press)
            .top_right_with_margins_on(state.ids.frame, 0.0, 0.0)
            .set(state.ids.close, ui)
            .was_clicked()
        {
            events.push(Event::Close);
        }

        Text::new(JOURNAL_TITLE)
            .mid_top_with_margin_on(state.ids.frame, 12.0)
            .font_id(self.fonts.cyri.conrod_id)
            .font_size(self.fonts.cyri.scale(28))
            .color(TEXT_COLOR)
            .set(state.ids.title, ui);
        Text::new("EMBERWATCH FRONTIER CONTRACTS")
            .mid_bottom_with_margin_on(state.ids.title, -2.0)
            .font_id(self.fonts.cyri.conrod_id)
            .font_size(self.fonts.cyri.scale(11))
            .color(COLOR_MUTED)
            .set(state.ids.subtitle, ui);

        Rectangle::fill_with([280.0, 60.0], COLOR_PANEL)
            .top_right_with_margins_on(state.ids.frame, 42.0, 18.0)
            .set(state.ids.standing_bg, ui);
        Rectangle::fill_with([260.0, 44.0], COLOR_CARD)
            .top_left_with_margins_on(state.ids.standing_bg, 8.0, 10.0)
            .set(state.ids.standing_fill, ui);
        let rep_max = 120.0;
        let rep_fraction = (progress.reputation.min(120) as f64 / rep_max).clamp(0.0, 1.0);
        Rectangle::fill_with([(244.0 * rep_fraction).max(1.0), 5.0], COLOR_EMBER)
            .top_left_with_margins_on(state.ids.standing_fill, 34.0, 8.0)
            .set(state.ids.standing_bar, ui);
        let standing_text = format!(
            "{}  •  {} standing  •  {} contracts",
            FACTION_NAME,
            self.rank(),
            progress.completed
        );
        Text::new(&standing_text)
            .top_left_with_margins_on(state.ids.standing_fill, 8.0, 8.0)
            .w(244.0)
            .font_id(self.fonts.cyri.conrod_id)
            .font_size(self.fonts.cyri.scale(12))
            .color(TEXT_COLOR)
            .set(state.ids.standing_text, ui);

        let board_label = board
            .map(|b| format!("{}  •  {:.0} m", BOARD_NAME, b.distance(self.player_pos)))
            .unwrap_or_else(|| "No Ember Board discovered".to_string());
        Text::new(&board_label)
            .top_left_with_margins_on(state.ids.frame, 48.0, 22.0)
            .w(430.0)
            .font_id(self.fonts.cyri.conrod_id)
            .font_size(self.fonts.cyri.scale(13))
            .color(COLOR_MUTED)
            .set(state.ids.board_text, ui);

        let content_x = 28.0;
        let content_y = 108.0;
        let active = self.state.active();
        let beacon_ready = self.state.beacon_ready(self.player_pos);

        if beacon_ready {
            Rectangle::fill_with([884.0, 132.0], COLOR_CARD)
                .top_left_with_margins_on(state.ids.frame, content_y, content_x)
                .set(state.ids.active_card, ui);
            Rectangle::fill_with([5.0, 132.0], COLOR_EMBER)
                .top_left_with_margins_on(state.ids.active_card, 0.0, 0.0)
                .set(state.ids.active_accent, ui);
            Text::new("THE EMBERWATCH BEACON")
                .top_left_with_margins_on(state.ids.active_card, 12.0, 18.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(19))
                .color(COLOR_EMBER)
                .set(state.ids.active_title, ui);
            Text::new(
                "Three contracts are complete. Light the beacon and turn Hearthpost into a \
                 permanent frontier waystation.",
            )
            .top_left_with_margins_on(state.ids.active_card, 42.0, 18.0)
            .w(650.0)
            .font_id(self.fonts.cyri.conrod_id)
            .font_size(self.fonts.cyri.scale(13))
            .color(TEXT_COLOR)
            .set(state.ids.active_description, ui);
            if self.button(
                "LIGHT THE BEACON",
                state.ids.active_card,
                state.ids.beacon_button,
                12.0,
                ui,
            ) {
                events.push(Event::FoundBeacon);
            }
            Text::new("+150 coins  •  +40 standing  •  permanent world change")
                .bottom_left_with_margins_on(state.ids.active_card, 12.0, 18.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(11))
                .color(COLOR_MUTED)
                .set(state.ids.beacon_text, ui);
        } else if let Some(active) = active {
            Rectangle::fill_with([884.0, 150.0], COLOR_CARD)
                .top_left_with_margins_on(state.ids.frame, content_y, content_x)
                .set(state.ids.active_card, ui);
            Rectangle::fill_with([5.0, 150.0], active.kind.color())
                .top_left_with_margins_on(state.ids.active_card, 0.0, 0.0)
                .set(state.ids.active_accent, ui);
            let active_kind = format!("ACTIVE  •  {}", active.kind.label());
            Text::new(&active_kind)
                .top_left_with_margins_on(state.ids.active_card, 10.0, 18.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(11))
                .color(active.kind.color())
                .set(state.ids.active_kind, ui);
            Text::new(active.kind.title())
                .top_left_with_margins_on(state.ids.active_card, 28.0, 18.0)
                .w(650.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(19))
                .color(TEXT_COLOR)
                .set(state.ids.active_title, ui);
            let active_desc = active_description(active, self.i18n);
            Text::new(&active_desc)
                .top_left_with_margins_on(state.ids.active_card, 56.0, 18.0)
                .w(650.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(12))
                .color(TEXT_COLOR)
                .set(state.ids.active_description, ui);
            let active_prog = active_progress(active, self.player_pos, board);
            Text::new(&active_prog)
                .top_left_with_margins_on(state.ids.active_card, 82.0, 18.0)
                .w(500.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(12))
                .color(COLOR_MUTED)
                .set(state.ids.active_progress, ui);
            let active_meta = format!(
                "Reward: {} coins  •  +{} standing",
                active.reward, active.reputation
            );
            Text::new(&active_meta)
                .top_left_with_margins_on(state.ids.active_card, 82.0, 500.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(11))
                .color(COLOR_EMBER)
                .set(state.ids.active_meta, ui);
            if self.button(
                "ABANDON CONTRACT",
                state.ids.active_card,
                state.ids.abandon_button,
                12.0,
                ui,
            ) {
                events.push(Event::Abandon);
            }
        } else if near_board {
            Text::new("AVAILABLE CONTRACTS")
                .top_left_with_margins_on(state.ids.frame, content_y, content_x)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(15))
                .color(COLOR_MUTED)
                .set(state.ids.section_title, ui);
            for (index, offer) in self.state.offers().iter().enumerate().take(3) {
                let x = content_x + index as f64 * 296.0;
                let y = content_y + 28.0;
                let card_w = 270.0;
                let card_h = 350.0;
                Rectangle::fill_with([card_w, card_h], COLOR_CARD)
                    .top_left_with_margins_on(state.ids.frame, y, x)
                    .set(state.ids.cards[index], ui);
                Rectangle::fill_with([5.0, card_h], offer.kind.color())
                    .top_left_with_margins_on(state.ids.cards[index], 0.0, 0.0)
                    .set(state.ids.card_accents[index], ui);
                Text::new(offer.kind.label())
                    .top_left_with_margins_on(state.ids.cards[index], 16.0, 18.0)
                    .font_id(self.fonts.cyri.conrod_id)
                    .font_size(self.fonts.cyri.scale(11))
                    .color(offer.kind.color())
                    .set(state.ids.card_kinds[index], ui);
                let title = offer_title(offer, self.i18n);
                Text::new(&title)
                    .top_left_with_margins_on(state.ids.cards[index], 40.0, 18.0)
                    .w(230.0)
                    .font_id(self.fonts.cyri.conrod_id)
                    .font_size(self.fonts.cyri.scale(19))
                    .color(TEXT_COLOR)
                    .set(state.ids.card_titles[index], ui);
                Text::new(offer.kind.description())
                    .top_left_with_margins_on(state.ids.cards[index], 82.0, 18.0)
                    .w(232.0)
                    .font_id(self.fonts.cyri.conrod_id)
                    .font_size(self.fonts.cyri.scale(12))
                    .color(TEXT_COLOR)
                    .set(state.ids.card_descriptions[index], ui);
                let meta = format!(
                    "{:.0} m from the board  •  {} coins  •  +{} standing",
                    offer.hint.distance(self.player_pos),
                    offer.kind.reward(),
                    offer.kind.reputation()
                );
                Text::new(&meta)
                    .bottom_left_with_margins_on(state.ids.cards[index], 58.0, 18.0)
                    .w(232.0)
                    .font_id(self.fonts.cyri.conrod_id)
                    .font_size(self.fonts.cyri.scale(11))
                    .color(COLOR_MUTED)
                    .set(state.ids.card_meta[index], ui);
                if self.button(
                    "ACCEPT CONTRACT",
                    state.ids.cards[index],
                    state.ids.card_buttons[index],
                    14.0,
                    ui,
                ) {
                    events.push(Event::Accept(index));
                }
            }
        } else {
            Text::new("THE BOARD IS OUT OF RANGE")
                .mid_top_with_margin_on(state.ids.frame, content_y + 40.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(20))
                .color(COLOR_EMBER)
                .set(state.ids.section_title, ui);
            let out_of_range = format!(
                "Return to {BOARD_NAME} to read new contracts. Active contracts continue to track \
                 while you travel."
            );
            Text::new(&out_of_range)
                .mid_top_with_margin_on(state.ids.section_title, 36.0)
                .w(650.0)
                .font_id(self.fonts.cyri.conrod_id)
                .font_size(self.fonts.cyri.scale(13))
                .color(TEXT_COLOR)
                .set(state.ids.empty_text, ui);
        }

        Text::new(if progress.world_change {
            "EMBERWATCH BEACON: Hearthpost is a permanent waystation. The Emberwatch will remember \
             this."
        } else {
            "Earn 3 contracts, then return here to light the Emberwatch Beacon."
        })
        .bottom_left_with_margins_on(state.ids.frame, 44.0, 28.0)
        .w(600.0)
        .font_id(self.fonts.cyri.conrod_id)
        .font_size(self.fonts.cyri.scale(12))
        .color(if progress.world_change {
            COLOR_EMBER
        } else {
            COLOR_MUTED
        })
        .set(state.ids.footer, ui);
        Text::new("U  Field Journal")
            .bottom_right_with_margins_on(state.ids.frame, 44.0, 26.0)
            .font_id(self.fonts.cyri.conrod_id)
            .font_size(self.fonts.cyri.scale(12))
            .color(COLOR_MUTED)
            .set(state.ids.key_hint, ui);

        events
    }
}

fn offer_title(offer: &Offer, i18n: &Localization) -> String {
    match &offer.data {
        OfferData::Bounty {
            target: Some(target),
            ..
        } => format!("Bounty: {}", i18n.get_content(&target.name)),
        _ => offer.kind.title().to_string(),
    }
}

fn active_description(active: &ActiveContract, i18n: &Localization) -> String {
    match &active.objective {
        Objective::Survey { .. } => {
            "Chart each marked site. The next site is highlighted on your map.".to_string()
        },
        Objective::Bounty { target, .. } => target
            .as_ref()
            .map(|target| {
                format!(
                    "Hunt down {} and return alive. Its last known position is marked on your map.",
                    i18n.get_content(&target.name)
                )
            })
            .unwrap_or_else(|| {
                "Follow the marked trail and confirm the threat is gone.".to_string()
            }),
        Objective::Courier { returning, .. } => {
            if *returning {
                "Report back to Hearthpost with the reply.".to_string()
            } else {
                "Reach the marked frontier relay, then return to Hearthpost.".to_string()
            }
        },
    }
}

fn active_progress(
    active: &ActiveContract,
    player_pos: Vec2<f32>,
    board: Option<Vec2<f32>>,
) -> String {
    match &active.objective {
        Objective::Survey { points, visited } => {
            let next = visited
                .iter()
                .enumerate()
                .find_map(|(i, seen)| (!*seen).then_some(i))
                .unwrap_or(0);
            format!(
                "Sites charted: {}/3  •  next site {:.0} m",
                visited.iter().filter(|seen| **seen).count(),
                points[next].distance(player_pos)
            )
        },
        Objective::Bounty { last_known, .. } => {
            format!("Last known trail: {:.0} m", last_known.distance(player_pos))
        },
        Objective::Courier {
            destination,
            returning,
        } => format!(
            "{}  •  {:.0} m",
            if *returning {
                "Report to Hearthpost"
            } else {
                "Frontier relay"
            },
            if *returning {
                board.map_or(0.0, |board| board.distance(player_pos))
            } else {
                destination.distance(player_pos)
            }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_completion(kind: ContractKind, objective: Objective) {
        let mut state = State::new(ContractProgress::default(), "Test".to_string(), None);
        state.active = Some(ActiveContract {
            kind,
            objective,
            reward: kind.reward(),
            reputation: kind.reputation(),
        });

        let actions = state.complete_active();

        assert_eq!(state.progress.completed, 1);
        assert_eq!(state.progress.reputation, kind.reputation());
        assert!(state.active.is_none());
        assert!(state.dirty);
        assert!(actions.iter().any(|action| matches!(
            action,
            Action::GrantCoins(amount) if *amount == kind.reward()
        )));
    }

    #[test]
    fn every_contract_kind_grants_its_reward_and_standing() {
        assert_completion(ContractKind::Survey, Objective::Survey {
            points: [Vec2::zero(); 3],
            visited: [true; 3],
        });
        assert_completion(ContractKind::Bounty, Objective::Bounty {
            target: None,
            last_known: Vec2::zero(),
            missing_for: 0.0,
        });
        assert_completion(ContractKind::Courier, Objective::Courier {
            destination: Vec2::zero(),
            returning: true,
        });
    }

    #[test]
    fn third_completion_announces_the_beacon() {
        let mut state = State::new(
            ContractProgress {
                completed: 2,
                ..ContractProgress::default()
            },
            "Test".to_string(),
            None,
        );
        state.active = Some(ActiveContract {
            kind: ContractKind::Survey,
            objective: Objective::Survey {
                points: [Vec2::zero(); 3],
                visited: [true; 3],
            },
            reward: ContractKind::Survey.reward(),
            reputation: ContractKind::Survey.reputation(),
        });

        let actions = state.complete_active();

        assert_eq!(state.progress.completed, 3);
        assert!(actions.iter().any(|action| matches!(
            action,
            Action::Message(message) if message.contains("ready for its Beacon")
        )));
    }
}
