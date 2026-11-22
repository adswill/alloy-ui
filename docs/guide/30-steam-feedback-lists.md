# Lists, rows and grids (Steam feedback, area L)

Rows of `vlist` and cells of `vgrid` are built at the same place, one scope per index, so they
now behave like ordinary views:

* **Events.** Rows always see pointer movement and ticks, so `hovered_for`, `.on_hover`,
  `.watch_hover` and `tick_while_shown` work inside them. While any popup is open, rows also see
  every key and press: Escape, arrows and Enter reach a row's `context_menu`, and a click on
  another row dismisses it (so two menus are never open at once). `vgrid` cells also take
  overlay presses (popovers and menus in cells).
* **Keys.** Each row is its own key scope; `keyed(id, ..)` inside a row is no longer needed for
  `context_menu` or `popover`.
* **Motion.** A row that is fading in (`reveal_appear`) keeps frames coming until it is done.

```ignore
vlist(rows.len(), |i| {
    let over = hovered_for(i as u64);
    row![text(&rows[i]), reveal_appear(over.get(), text("Play"))].watch_hover(over)
})
.row_height(36.0)
```

## vgrid: focus ring and Enter

`.focus_ring(false)` hides the ring drawn around the whole grid. `.on_activate(|s, i| ..)` runs
on Enter for the highlighted cell; arrow keys still only call `.act`.

```ignore
vgrid(items.len(), 4, |i| tile(&items[i]))
    .focus_ring(false)
    .act(|s: &mut State, i| s.highlight = Some(i))
    .on_activate(|s: &mut State, i| s.open(i))
```

## Carousel

* The default height is that of the first slide (at least 96 px), not the tallest or a whole
  page; `.height(px)` still overrides.
* `carousel_with(count, |i| slide)` builds only the first, current, next and a fading-out slide.
* While the pointer is over an `.auto(..)` carousel it no longer asks for frames; moving away
  restarts the timer.

```ignore
carousel_with(shots.len(), |i| image(&shots[i])).auto(secs(5.0))
```

Limit: the runtime has no "wake me at time T" call, so an `.auto` carousel not under the pointer
still ticks every frame. A true single wake per advance needs a runtime deadline.
