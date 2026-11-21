//! Planner: a kanban board and a calendar, both drag and drop, both covered by undo.
use crate::data::TODAY;
use crate::ui::*;
use crate::Studio;
use alloy_ui::i18n;
use alloy_ui::prelude::*;
use alloy_ui::{entry, CalAction, CalView};

fn calendar_action(s: &mut Studio, a: CalAction) {
    let step = |v: CalView| match v {
        CalView::Month => 0,
        CalView::Week => 7,
        CalView::Day => 1,
    };
    match a {
        CalAction::Prev | CalAction::Next => {
            let dir = if a == CalAction::Next { 1 } else { -1 };
            s.day = match s.cal_view {
                CalView::Month => s.day.add_months(dir),
                v => alloy_ui::widgets::Date::from_days(s.day.to_days() + dir as i64 * step(v)),
            };
        }
        CalAction::Today => s.day = TODAY,
        CalAction::View(v) => s.cal_view = v,
        CalAction::Day(d) => {
            s.day = d;
            s.cal_view = CalView::Day;
        }
        CalAction::Create { date, minute } => {
            let id = s.work.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
            s.work
                .entries
                .push(entry(id, "New event", date, minute, minute + 60));
            toast("Event created").undo();
        }
        CalAction::Move { id, date, start } => {
            if let Some(e) = s.work.entries.iter_mut().find(|e| e.id == id) {
                let dur = e.end - e.start;
                (e.date, e.start, e.end) = (date, start, start + dur);
            }
        }
        CalAction::Resize { id, end } => {
            if let Some(e) = s.work.entries.iter_mut().find(|e| e.id == id) {
                e.end = end.max(e.start + 15);
            }
        }
        CalAction::Open(_) => {}
    }
}

pub fn view(s: &Studio) -> N {
    let body: N = match s.planner_tab {
        0 => col![
            caption(match s.opened_card {
                Some(id) => format!("Opened card {id}. Drag cards between columns, or focus the board and use Alt+arrows."),
                None => "Drag cards between columns, or focus the board and use Alt+arrows. Cmd/Ctrl+Z undoes a move.".into(),
            })
            .dim(),
            kanban(&s.work.lanes)
                .act(|s: &mut Studio, mv| mv.apply(&mut s.work.lanes))
                .open(|s: &mut Studio, id| s.opened_card = Some(id)),
        ]
        .gap(10.0)
        .into(),
        _ => calendar(s.day, s.cal_view, &s.work.entries)
            .today(TODAY)
            .now(13 * 60 + 5)
            .hours(7, 19)
            .act(calendar_action)
            .into(),
    };
    fixed(col![
        header(
            i18n::translate("pl.title", &[]),
            i18n::translate("pl.sub", &[]),
            button("Undo").act(|_| undo())
        ),
        tab_bar(["Board", "Calendar"], at!(s.planner_tab)),
        body.fill_h(),
    ])
}
