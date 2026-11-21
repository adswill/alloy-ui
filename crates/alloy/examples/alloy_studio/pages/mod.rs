//! One module per page. `page` picks the one the router is showing.
use crate::ui::N;
use crate::{Route, Studio};
use alloy_ui::prelude::*;

pub mod appearance;
pub mod canvas;
pub mod data;
pub mod editors;
pub mod forms;
pub mod motion;
pub mod overview;
pub mod planner;
pub mod system;
pub mod workbench;

pub fn page(s: &Studio, route: Route, nav: Router<Route>) -> N {
    match route {
        Route::Overview => overview::view(s),
        Route::Data => data::view(s, nav),
        Route::Customer(id) => data::customer(s, id, nav),
        Route::Editors => editors::view(s),
        Route::Planner => planner::view(s),
        Route::Canvas => canvas::view(s),
        Route::Forms => forms::view(s),
        Route::Workbench => workbench::view(s),
        Route::Motion => motion::view(s),
        Route::Appearance => appearance::view(s),
        Route::System => system::view(s),
    }
}
