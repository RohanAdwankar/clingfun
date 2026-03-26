use std::fmt::Write as _;
use std::time::Duration;

use eframe::egui;
use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2,
    pos2, vec2,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use web_time::Instant;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([720.0, 540.0]),
        ..Default::default()
    };

    eframe::run_native(
        "clingfun",
        options,
        Box::new(|cc| Ok(Box::new(ClingfunApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start_web() -> Result<(), JsValue> {
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();
    let runner = eframe::WebRunner::new();
    let window = web_sys::window().expect("window unavailable");
    let document = window.document().expect("document unavailable");
    let canvas = document
        .get_element_by_id("clingfun-canvas")
        .expect("missing canvas element")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("element is not a canvas");

    runner
        .start(
            canvas,
            web_options,
            Box::new(|cc| Ok(Box::new(ClingfunApp::new(cc)))),
        )
        .await?;

    if let Some(loading) = document.get_element_by_id("loading") {
        loading.set_inner_html("");
        let _ = loading.set_attribute("style", "display: none;");
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}

struct ClingfunApp {
    state: GameState,
}

impl ClingfunApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: GameState::new(),
        }
    }
}

impl eframe::App for ClingfunApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.handle_keyboard(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.heading("clingfun");
                ui.label("Infinite bite-sized language puzzles");
            });
            ui.add_space(8.0);

            ui.columns(2, |columns| {
                columns[0].group(|ui| {
                    ui.heading("Score");
                    ui.label(format!("Points: {}", self.state.score));
                    ui.label(format!("Solved: {}", self.state.solved));
                    ui.label(format!("Streak: {}", self.state.streak));
                    ui.label(format!("Best streak: {}", self.state.best_streak));
                });

                columns[1].group(|ui| {
                    ui.heading("Round");
                    ui.label(format!("Type: {}", self.state.puzzle.kind_name()));
                    ui.label(format!(
                        "Timer: {:.1}s",
                        self.state.round_started.elapsed().as_secs_f32()
                    ));
                    ui.label("Answer with Y / N or click the buttons below.");
                });
            });

            ui.add_space(16.0);
            ui.group(|ui| {
                ui.heading(self.state.puzzle.title());
                ui.separator();
                self.state.puzzle.show(ui);
            });

            ui.add_space(12.0);
            ui.group(|ui| {
                ui.heading("Question");
                ui.label(format!(
                    "Does the string {:?} belong to the language?",
                    self.state.puzzle.test_string()
                ));
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let yes = ui.add_sized([160.0, 44.0], egui::Button::new("Yes").shortcut_text("Y"));
                if yes.clicked() {
                    self.state.answer(true);
                }

                let no = ui.add_sized([160.0, 44.0], egui::Button::new("No").shortcut_text("N"));
                if no.clicked() {
                    self.state.answer(false);
                }

                if ui.button("Skip").clicked() {
                    self.state.skip();
                }
            });

            if let Some(message) = &self.state.feedback {
                ui.add_space(12.0);
                ui.group(|ui| {
                    ui.heading("Last result");
                    ui.label(message);
                });
            }
        });

        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

struct GameState {
    rng: SimpleRng,
    puzzle: Puzzle,
    round_started: Instant,
    score: u32,
    solved: u32,
    streak: u32,
    best_streak: u32,
    feedback: Option<String>,
}

impl GameState {
    fn new() -> Self {
        let mut rng = SimpleRng::seeded();
        let puzzle = Puzzle::generate(&mut rng);
        Self {
            rng,
            puzzle,
            round_started: Instant::now(),
            score: 0,
            solved: 0,
            streak: 0,
            best_streak: 0,
            feedback: None,
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.key_pressed(egui::Key::Y)) {
            self.answer(true);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::N)) {
            self.answer(false);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
            self.skip();
        }
    }

    fn answer(&mut self, guess: bool) {
        let correct = self.puzzle.is_match();
        let elapsed = self.round_started.elapsed();

        if guess == correct {
            let awarded = score_for_time(elapsed);
            self.score += awarded;
            self.solved += 1;
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);
            self.feedback = Some(format!(
                "Correct. +{} points in {:.1}s. {}",
                awarded,
                elapsed.as_secs_f32(),
                self.puzzle.explanation()
            ));
        } else {
            self.streak = 0;
            self.feedback = Some(format!(
                "Wrong after {:.1}s. Correct answer: {}. {}",
                elapsed.as_secs_f32(),
                yes_no(correct),
                self.puzzle.explanation()
            ));
        }

        self.next_round();
    }

    fn skip(&mut self) {
        self.streak = 0;
        self.feedback = Some(format!(
            "Skipped. Correct answer: {}. {}",
            yes_no(self.puzzle.is_match()),
            self.puzzle.explanation()
        ));
        self.next_round();
    }

    fn next_round(&mut self) {
        self.puzzle = Puzzle::generate(&mut self.rng);
        self.round_started = Instant::now();
    }
}

fn score_for_time(elapsed: Duration) -> u32 {
    let secs = elapsed.as_secs_f32();
    if secs < 2.0 {
        25
    } else if secs < 4.0 {
        18
    } else if secs < 7.0 {
        12
    } else if secs < 11.0 {
        8
    } else {
        5
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

enum Puzzle {
    Dfa(DfaPuzzle),
    Cfg(CfgPuzzle),
}

impl Puzzle {
    fn generate(rng: &mut SimpleRng) -> Self {
        if rng.gen_bool() {
            Self::Dfa(DfaPuzzle::generate(rng))
        } else {
            Self::Cfg(CfgPuzzle::generate(rng))
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Dfa(_) => "FSA",
            Self::Cfg(_) => "CFG",
        }
    }

    fn title(&self) -> &'static str {
        match self {
            Self::Dfa(_) => "Finite State Automaton",
            Self::Cfg(_) => "Context-Free Grammar",
        }
    }

    fn show(&self, ui: &mut egui::Ui) {
        match self {
            Self::Dfa(puzzle) => puzzle.show(ui),
            Self::Cfg(puzzle) => {
                ui.monospace(puzzle.render());
            }
        }
    }

    fn test_string(&self) -> &str {
        match self {
            Self::Dfa(puzzle) => &puzzle.input,
            Self::Cfg(puzzle) => &puzzle.input,
        }
    }

    fn is_match(&self) -> bool {
        match self {
            Self::Dfa(puzzle) => puzzle.is_match(),
            Self::Cfg(puzzle) => puzzle.is_match(),
        }
    }

    fn explanation(&self) -> String {
        match self {
            Self::Dfa(puzzle) => puzzle.explanation(),
            Self::Cfg(puzzle) => puzzle.explanation(),
        }
    }
}

struct DfaPuzzle {
    transitions: Vec<[usize; 2]>,
    accepting: Vec<bool>,
    input: String,
    trace: Vec<usize>,
}

impl DfaPuzzle {
    fn generate(rng: &mut SimpleRng) -> Self {
        let states = rng.gen_range(3, 6);
        let mut transitions = Vec::with_capacity(states);
        for _ in 0..states {
            transitions.push([rng.gen_range(0, states), rng.gen_range(0, states)]);
        }

        let mut accepting = vec![false; states];
        for state in 0..states {
            accepting[state] = rng.gen_ratio(2, 5);
        }

        if !accepting.iter().any(|&value| value) {
            accepting[rng.gen_range(0, states)] = true;
        }
        if accepting.iter().all(|&value| value) {
            accepting[rng.gen_range(0, states)] = false;
        }

        let input_len = rng.gen_range(3, 9);
        let input = random_binary_string(rng, input_len);
        let trace = run_dfa(&transitions, &input);

        Self {
            transitions,
            accepting,
            input,
            trace,
        }
    }

    fn is_match(&self) -> bool {
        self.trace
            .last()
            .copied()
            .map(|state| self.accepting[state])
            .unwrap_or(false)
    }

    fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Alphabet: {0, 1}\nStart state: q0\nAccepting: ");

        let accepting_states: Vec<String> = self
            .accepting
            .iter()
            .enumerate()
            .filter_map(|(idx, &is_accepting)| is_accepting.then_some(format!("q{idx}")))
            .collect();
        out.push_str(&accepting_states.join(", "));
        out.push('\n');
        out.push('\n');
        out.push_str("Transitions\n");
        for (state, [on_zero, on_one]) in self.transitions.iter().enumerate() {
            let _ = writeln!(out, "  q{state}: 0 -> q{on_zero}, 1 -> q{on_one}");
        }
        out
    }

    fn show(&self, ui: &mut egui::Ui) {
        ui.label("Alphabet: {0, 1}. Start state: q0.");

        let desired_size = vec2(ui.available_width().max(320.0), 320.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());
        let painter = ui.painter_at(rect);
        self.paint(&painter, rect);

        ui.add_space(6.0);
        ui.collapsing("Transition table", |ui| {
            ui.monospace(self.render());
        });
    }

    fn paint(&self, painter: &egui::Painter, rect: Rect) {
        let radius = 26.0;
        let node_count = self.transitions.len();
        let positions = layout_positions(rect, node_count);

        painter.rect_filled(rect.shrink(4.0), 10.0, Color32::from_rgb(15, 27, 42));

        for source in 0..node_count {
            for symbol_idx in 0..2 {
                let target = self.transitions[source][symbol_idx];
                let label = if symbol_idx == 0 { "0" } else { "1" };
                let label_text = if self.transitions[source][0] == self.transitions[source][1]
                    && symbol_idx == 0
                {
                    "0,1"
                } else if self.transitions[source][0] == self.transitions[source][1] {
                    continue;
                } else {
                    label
                };

                if source == target {
                    draw_self_loop(
                        painter,
                        positions[source],
                        radius,
                        loop_direction(source),
                        label_text,
                        Color32::from_rgb(139, 197, 255),
                    );
                } else {
                    let reverse_exists = self.transitions[target][0] == source
                        || self.transitions[target][1] == source;
                    let bend = if reverse_exists {
                        if source < target { 34.0 } else { -34.0 }
                    } else {
                        0.0
                    };
                    draw_directed_edge(
                        painter,
                        positions[source],
                        positions[target],
                        radius,
                        bend,
                        label_text,
                        Color32::from_rgb(139, 197, 255),
                    );
                }
            }
        }

        draw_start_arrow(
            painter,
            positions[0],
            radius,
            Color32::from_rgb(255, 214, 102),
        );

        for (idx, position) in positions.into_iter().enumerate() {
            let fill = if idx == 0 {
                Color32::from_rgb(36, 63, 92)
            } else {
                Color32::from_rgb(27, 45, 66)
            };
            painter.circle_filled(position, radius, fill);
            painter.circle_stroke(position, radius, Stroke::new(2.0, Color32::WHITE));
            if self.accepting[idx] {
                painter.circle_stroke(position, radius - 5.0, Stroke::new(2.0, Color32::WHITE));
            }
            painter.text(
                position,
                Align2::CENTER_CENTER,
                format!("q{idx}"),
                FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
    }

    fn explanation(&self) -> String {
        let path = self
            .trace
            .iter()
            .map(|state| format!("q{state}"))
            .collect::<Vec<_>>()
            .join(" -> ");
        format!(
            "Trace: {}. Final state {} is {}accepting.",
            path,
            self.trace.last().copied().unwrap_or(0),
            if self.is_match() { "" } else { "not " }
        )
    }
}

fn draw_start_arrow(painter: &egui::Painter, target: Pos2, radius: f32, color: Color32) {
    let start = target + vec2(-radius - 42.0, 0.0);
    let end = target + vec2(-radius, 0.0);
    painter.line_segment([start, end], Stroke::new(2.0, color));
    draw_arrow_head(painter, end, vec2(1.0, 0.0), color);
}

fn draw_self_loop(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    outward: Vec2,
    label: &str,
    color: Color32,
) {
    let tangent = vec2(-outward.y, outward.x);
    let anchor = center + outward * (radius + 6.0);
    let p0 = anchor + tangent * 8.0;
    let p1 = center + outward * (radius + 36.0) + tangent * 26.0;
    let p2 = center + outward * (radius + 50.0);
    let p3 = center + outward * (radius + 36.0) - tangent * 26.0;
    let p4 = anchor - tangent * 8.0;
    let points = cubic_points(p0, p1, p2, p3, p4, 28);
    painter.add(Shape::line(points, Stroke::new(2.0, color)));
    draw_arrow_head(painter, p4, p4 - p3, color);
    draw_edge_label(painter, p2 + outward * 15.0, label, color);
}

fn draw_directed_edge(
    painter: &egui::Painter,
    source: Pos2,
    target: Pos2,
    radius: f32,
    bend: f32,
    label: &str,
    color: Color32,
) {
    let delta = target - source;
    let distance = delta.length().max(1.0);
    let dir = delta / distance;
    let perp = vec2(-dir.y, dir.x);
    let start = source + dir * radius;
    let end = target - dir * radius;
    let control = pos2((start.x + end.x) * 0.5, (start.y + end.y) * 0.5) + perp * bend;
    let curve = quadratic_points(start, control, end, 24);
    let label_anchor =
        quadratic_point(start, control, end, 0.5) + perp * if bend == 0.0 { 12.0 } else { 14.0 };

    painter.add(Shape::line(curve.clone(), Stroke::new(2.0, color)));
    let before_tip = curve[curve.len() - 2];
    draw_arrow_head(painter, end, end - before_tip, color);
    draw_edge_label(painter, label_anchor, label, color);
}

fn draw_arrow_head(painter: &egui::Painter, tip: Pos2, direction: Vec2, color: Color32) {
    let dir = direction.normalized();
    let wing = vec2(-dir.y, dir.x);
    let size = 9.0;
    let left = tip - dir * size + wing * (size * 0.55);
    let right = tip - dir * size - wing * (size * 0.55);
    painter.line_segment([left, tip], Stroke::new(2.0, color));
    painter.line_segment([right, tip], Stroke::new(2.0, color));
}

fn draw_edge_label(painter: &egui::Painter, center: Pos2, label: &str, color: Color32) {
    let rect = Rect::from_center_size(center, vec2((label.len() as f32 * 8.0).max(22.0), 20.0));
    painter.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(15, 27, 42));
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
        StrokeKind::Outside,
    );
    painter.text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(15.0),
        color,
    );
}

fn layout_positions(rect: Rect, node_count: usize) -> Vec<Pos2> {
    let center = pos2(rect.center().x, rect.center().y + 6.0);
    let presets: &[&[(f32, f32)]] = &[
        &[],
        &[(0.0, 0.0)],
        &[(-0.24, 0.08), (0.24, -0.08)],
        &[(0.0, -0.26), (-0.28, 0.18), (0.28, 0.18)],
        &[(-0.25, -0.2), (0.25, -0.2), (-0.25, 0.2), (0.25, 0.2)],
        &[
            (0.0, -0.3),
            (-0.28, -0.04),
            (0.28, -0.04),
            (-0.19, 0.27),
            (0.19, 0.27),
        ],
    ];

    if let Some(points) = presets.get(node_count) {
        return points
            .iter()
            .map(|(x, y)| pos2(center.x + x * rect.width(), center.y + y * rect.height()))
            .collect();
    }

    let orbit = rect.width().min(rect.height()).min(220.0) * 0.38;
    (0..node_count)
        .map(|idx| {
            let angle = std::f32::consts::TAU * idx as f32 / node_count as f32
                - std::f32::consts::FRAC_PI_2;
            pos2(
                center.x + orbit * angle.cos(),
                center.y + orbit * angle.sin(),
            )
        })
        .collect()
}

fn loop_direction(index: usize) -> Vec2 {
    match index % 5 {
        0 => vec2(0.8, -0.6).normalized(),
        1 => vec2(1.0, 0.1).normalized(),
        2 => vec2(-0.8, -0.6).normalized(),
        3 => vec2(-1.0, 0.1).normalized(),
        _ => vec2(0.0, 1.0),
    }
}

fn quadratic_point(start: Pos2, control: Pos2, end: Pos2, t: f32) -> Pos2 {
    let mt = 1.0 - t;
    pos2(
        mt * mt * start.x + 2.0 * mt * t * control.x + t * t * end.x,
        mt * mt * start.y + 2.0 * mt * t * control.y + t * t * end.y,
    )
}

fn quadratic_points(start: Pos2, control: Pos2, end: Pos2, steps: usize) -> Vec<Pos2> {
    (0..=steps)
        .map(|step| quadratic_point(start, control, end, step as f32 / steps as f32))
        .collect()
}

fn cubic_point(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;
    pos2(
        mt2 * mt * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t2 * t * p3.x,
        mt2 * mt * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t2 * t * p3.y,
    )
}

fn cubic_points(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, p4: Pos2, steps: usize) -> Vec<Pos2> {
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        if t <= 0.5 {
            points.push(cubic_point(p0, p1, p2, p2, t * 2.0));
        } else {
            points.push(cubic_point(p2, p2, p3, p4, (t - 0.5) * 2.0));
        }
    }
    points
}

fn run_dfa(transitions: &[[usize; 2]], input: &str) -> Vec<usize> {
    let mut state = 0;
    let mut trace = vec![state];
    for ch in input.chars() {
        let index = if ch == '0' { 0 } else { 1 };
        state = transitions[state][index];
        trace.push(state);
    }
    trace
}

struct CfgPuzzle {
    grammar: GrammarFamily,
    input: String,
    is_match: bool,
}

impl CfgPuzzle {
    fn generate(rng: &mut SimpleRng) -> Self {
        let grammar = GrammarFamily::generate(rng);
        let should_match = rng.gen_bool();
        let input = if should_match {
            grammar.generate_member(rng)
        } else {
            grammar.generate_non_member(rng)
        };
        let is_match = grammar.matches(&input);

        Self {
            grammar,
            input,
            is_match,
        }
    }

    fn is_match(&self) -> bool {
        self.is_match
    }

    fn render(&self) -> String {
        self.grammar.render()
    }

    fn explanation(&self) -> String {
        self.grammar.explanation(&self.input)
    }
}

enum GrammarFamily {
    BalancedPairs,
    Mirror,
    EvenZerosOddOnes,
}

impl GrammarFamily {
    fn generate(rng: &mut SimpleRng) -> Self {
        match rng.gen_range(0, 3) {
            0 => Self::BalancedPairs,
            1 => Self::Mirror,
            _ => Self::EvenZerosOddOnes,
        }
    }

    fn render(&self) -> String {
        match self {
            Self::BalancedPairs => "Alphabet: {a, b}\nRules:\n  S -> a S b | a b".to_string(),
            Self::Mirror => "Alphabet: {0, 1}\nRules:\n  S -> 0 S 0 | 1 S 1 | 0 | 1".to_string(),
            Self::EvenZerosOddOnes => {
                "Alphabet: {0, 1}\nRules:\n  S -> 0 A\n  A -> 0 S | 1 B | ε\n  B -> 1 A".to_string()
            }
        }
    }

    fn matches(&self, input: &str) -> bool {
        match self {
            Self::BalancedPairs => {
                let chars: Vec<char> = input.chars().collect();
                !chars.is_empty()
                    && chars.len() % 2 == 0
                    && chars.iter().take(chars.len() / 2).all(|&ch| ch == 'a')
                    && chars.iter().skip(chars.len() / 2).all(|&ch| ch == 'b')
                    && chars.len() / 2 >= 1
            }
            Self::Mirror => {
                let chars: Vec<char> = input.chars().collect();
                !chars.is_empty()
                    && chars.len() % 2 == 1
                    && chars
                        .iter()
                        .zip(chars.iter().rev())
                        .all(|(left, right)| left == right)
            }
            Self::EvenZerosOddOnes => {
                let zeros = input.chars().filter(|&ch| ch == '0').count();
                let ones = input.chars().filter(|&ch| ch == '1').count();
                !input.is_empty()
                    && input.chars().all(|ch| ch == '0' || ch == '1')
                    && zeros % 2 == 0
                    && ones % 2 == 1
            }
        }
    }

    fn generate_member(&self, rng: &mut SimpleRng) -> String {
        match self {
            Self::BalancedPairs => {
                let n = rng.gen_range(1, 5);
                format!("{}{}", "a".repeat(n), "b".repeat(n))
            }
            Self::Mirror => {
                let half = rng.gen_range(0, 4);
                let mut left = String::new();
                for _ in 0..half {
                    left.push(if rng.gen_bool() { '0' } else { '1' });
                }
                let center = if rng.gen_bool() { '0' } else { '1' };
                let right: String = left.chars().rev().collect();
                format!("{left}{center}{right}")
            }
            Self::EvenZerosOddOnes => {
                let zero_pairs = rng.gen_range(0, 4) * 2;
                let one_count = rng.gen_range(0, 4) * 2 + 1;
                shuffled_binary_string(rng, zero_pairs, one_count)
            }
        }
    }

    fn generate_non_member(&self, rng: &mut SimpleRng) -> String {
        for _ in 0..32 {
            let candidate = match self {
                Self::BalancedPairs => {
                    let length = rng.gen_range(1, 9);
                    random_ab_string(rng, length)
                }
                Self::Mirror => {
                    let length = rng.gen_range(1, 8);
                    random_binary_string(rng, length)
                }
                Self::EvenZerosOddOnes => {
                    let length = rng.gen_range(1, 8);
                    random_binary_string(rng, length)
                }
            };
            if !self.matches(&candidate) {
                return candidate;
            }
        }

        match self {
            Self::BalancedPairs => "aab".to_string(),
            Self::Mirror => "011".to_string(),
            Self::EvenZerosOddOnes => "00".to_string(),
        }
    }

    fn explanation(&self, input: &str) -> String {
        match self {
            Self::BalancedPairs => {
                let count_a = input.chars().take_while(|&ch| ch == 'a').count();
                let count_b = input.chars().rev().take_while(|&ch| ch == 'b').count();
                format!(
                    "This grammar generates exactly a^n b^n for n >= 1. Here a-count = {}, b-count = {}.",
                    count_a, count_b
                )
            }
            Self::Mirror => {
                let reversed: String = input.chars().rev().collect();
                format!(
                    "This grammar generates odd-length palindromes. Reverse({:?}) = {:?}.",
                    input, reversed
                )
            }
            Self::EvenZerosOddOnes => {
                let zeros = input.chars().filter(|&ch| ch == '0').count();
                let ones = input.chars().filter(|&ch| ch == '1').count();
                format!(
                    "This grammar generates strings with an even number of 0s and an odd number of 1s. Counts: 0s = {}, 1s = {}.",
                    zeros, ones
                )
            }
        }
    }
}

fn random_binary_string(rng: &mut SimpleRng, len: usize) -> String {
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        out.push(if rng.gen_bool() { '0' } else { '1' });
    }
    out
}

fn random_ab_string(rng: &mut SimpleRng, len: usize) -> String {
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        out.push(if rng.gen_bool() { 'a' } else { 'b' });
    }
    out
}

fn shuffled_binary_string(rng: &mut SimpleRng, zero_count: usize, one_count: usize) -> String {
    let mut chars = Vec::with_capacity(zero_count + one_count);
    chars.extend(std::iter::repeat_n('0', zero_count));
    chars.extend(std::iter::repeat_n('1', one_count));
    for idx in (1..chars.len()).rev() {
        let swap_idx = rng.gen_range(0, idx + 1);
        chars.swap(idx, swap_idx);
    }
    chars.into_iter().collect()
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn seeded() -> Self {
        Self {
            state: seed_entropy() ^ 0xa076_1d64_78bd_642f,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state = self.state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        self.state
    }

    fn gen_bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }

    fn gen_range(&mut self, start: usize, end: usize) -> usize {
        debug_assert!(start < end);
        start + (self.next_u64() as usize % (end - start))
    }

    fn gen_ratio(&mut self, numerator: usize, denominator: usize) -> bool {
        self.gen_range(0, denominator) < numerator
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn seed_entropy() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
}

#[cfg(target_arch = "wasm32")]
fn seed_entropy() -> u64 {
    // `SystemTime::now` is not available on wasm in this target, so seed from browser time.
    (js_sys::Date::now().to_bits() as u64) ^ 0x517c_c1b7_2722_0a95
}
