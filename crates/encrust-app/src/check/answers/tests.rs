use core_geometry::Vec3;

use super::*;
use crate::scene::Scene;
use crate::scene::fixtures::scene_of_boxes;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::tool_settings::ToolSettings;
use crate::undo::Frame;

const SETTLED: Frame = Frame {
    settled: true,
    typing: false,
};

struct Bench {
    scene: Scene,
    tools: Tools,
    slicing: Slicing,
    history: History,
    answers: Answers,
}

impl Bench {
    fn new() -> Self {
        let scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::ONE)]);
        let mut bench = Self {
            scene,
            tools: Tools::default(),
            slicing: Slicing::default(),
            history: History::default(),
            answers: Answers::default(),
        };
        bench.record();
        bench
    }

    fn record(&mut self) {
        let settings = ToolSettings::of(&self.tools, &self.slicing);
        let chosen = self.slicing.chosen();
        self.history
            .observe(&self.scene, &settings, chosen, SETTLED);
    }

    fn moved(&mut self, by: f32) {
        self.scene.objects_mut()[0].transform.translation.x += by;
    }

    fn fix(&mut self, by: f32, kind: IssueKind) {
        self.moved(by);
        let changed = self.history.has_unrecorded(&self.scene);
        let said = Status::Info("done".to_owned());
        self.answers.note(&self.history, changed, kind, &said);
        self.record();
    }

    fn standing(&mut self) -> Vec<IssueKind> {
        let answers = self.answers.standing(&self.history);
        answers.iter().map(|answer| answer.kind).collect()
    }

    fn undo(&mut self) {
        let done = self
            .history
            .undo(&mut self.scene, &mut self.tools, &mut self.slicing);
        assert!(done, "there is an edit to take back");
    }
}

#[test]
fn a_fix_that_changed_nothing_is_no_answer() {
    let mut bench = Bench::new();
    let said = Status::Info("no room".to_owned());
    let changed = bench.history.has_unrecorded(&bench.scene);
    bench
        .answers
        .note(&bench.history, changed, IssueKind::Cups, &said);
    assert!(bench.standing().is_empty());
}

#[test]
fn an_answer_stands_from_the_frame_of_its_press() {
    let mut bench = Bench::new();
    bench.moved(1.0);
    let said = Status::Info("done".to_owned());
    let changed = bench.history.has_unrecorded(&bench.scene);
    bench
        .answers
        .note(&bench.history, changed, IssueKind::Cups, &said);
    assert_eq!(bench.standing(), [IssueKind::Cups], "before it is recorded");
    bench.record();
    assert_eq!(bench.standing(), [IssueKind::Cups], "once it is recorded");
}

#[test]
fn fixes_stand_together_and_only_the_last_is_one_undo_away() {
    let mut bench = Bench::new();
    bench.fix(1.0, IssueKind::InTheWay);
    bench.fix(1.0, IssueKind::Islands);
    let answers = bench.answers.standing(&bench.history);
    assert_eq!(answers.len(), 2);
    assert!(!Answers::is_last(&answers[0], &bench.history));
    assert!(Answers::is_last(&answers[1], &bench.history));
}

#[test]
fn an_undone_answer_goes_and_its_redo_brings_it_back() {
    let mut bench = Bench::new();
    bench.fix(1.0, IssueKind::InTheWay);
    bench.fix(1.0, IssueKind::Islands);
    bench.undo();
    assert_eq!(bench.standing(), [IssueKind::InTheWay]);
    assert!(
        bench
            .history
            .redo(&mut bench.scene, &mut bench.tools, &mut bench.slicing)
    );
    assert_eq!(bench.standing(), [IssueKind::InTheWay, IssueKind::Islands]);
}

#[test]
fn an_edit_of_the_users_own_ends_what_the_fixes_said() {
    let mut bench = Bench::new();
    bench.fix(1.0, IssueKind::InTheWay);
    bench.moved(5.0);
    bench.record();
    assert!(bench.standing().is_empty());
}
