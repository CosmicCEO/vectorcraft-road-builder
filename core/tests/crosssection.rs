use road_builder_core::crosssection::{Anchor, Dir, format, layout, parse};
use road_builder_core::library::Library;

/// The owner's first story: "1 bike lane, 1 apron, 2 south lanes, a 3 m grassy curbed margin, 2 north lanes, apron, and a bike lane".
const STORY: &str = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";

#[test]
fn the_story_cross_section_parses_and_prints_back_the_same() {
    let lib = Library::builtin();
    let p = parse(STORY, &lib).unwrap();
    assert_eq!(p.items.len(), 9);
    assert_eq!(format(&p), STORY);
    assert!(p.notes.is_empty());
}

#[test]
fn plain_words_work_and_guesses_are_reported() {
    let lib = Library::builtin();
    let p = parse("bike lane, apron, 2 south lanes, 3 m grassy curbed margin", &lib);
    // "2 south lanes" is not the shorthand (a repeat is written x2), so the words are read as far as they go
    let p = p.unwrap();
    assert!(p.items.iter().any(|i| i.kind == "margin" && i.width == Some(3.0)), "{p:?}");
    assert!(p.notes.iter().any(|n| n.contains("grassy curbed margin") && n.contains("margin")), "{:?}", p.notes);
}

#[test]
fn an_unknown_element_gets_a_helpful_error() {
    let lib = Library::builtin();
    let e = parse("lane | trolley-track | lane", &lib).unwrap_err();
    assert!(e.contains("unknown element \"trolley-track\""), "{e}");
    assert!(e.contains("known elements"), "{e}");
    let e = parse("lane | bike", &lib).map(|_| ()).err();
    assert!(e.is_none(), "aliases resolve");
    assert!(parse("", &lib).is_err());
}

#[test]
fn widths_and_units() {
    let lib = Library::builtin();
    let p = parse("lane 12ft | lane 3.2m | shoulder 2", &lib).unwrap();
    assert!((p.items[0].width.unwrap() - 3.6576).abs() < 1e-9);
    assert_eq!(p.items[1].width, Some(3.2));
    assert_eq!(p.items[2].width, Some(2.0));
}

#[test]
fn layout_adds_up_and_centres_on_the_path() {
    let lib = Library::builtin();
    let placed = layout(&parse(STORY, &lib).unwrap(), &lib, [0.0, 1.0], Anchor::Center).unwrap();
    // 1.5 + 0.6 + 2*3.6 + 3.0 + 2*3.6 + 0.6 + 1.5
    let total = placed.last().unwrap().right - placed[0].left;
    assert!((total - 21.6).abs() < 1e-9, "{total}");
    assert!((placed[0].left + 10.8).abs() < 1e-9 && (placed.last().unwrap().right - 10.8).abs() < 1e-9);
    assert!(placed.windows(2).all(|w| (w[0].right - w[1].left).abs() < 1e-9), "elements touch, no gaps");
}

#[test]
fn compass_words_become_directions_along_the_path() {
    let lib = Library::builtin();
    let p = parse(STORY, &lib).unwrap();
    // Path drawn north: the south lanes (left, west side) run against it; the north lanes with it.
    let north = layout(&p, &lib, [0.0, 1.0], Anchor::Center).unwrap();
    let lanes: Vec<_> = north.iter().filter(|e| e.id == "lane").collect();
    assert_eq!(lanes.iter().map(|l| l.dir).collect::<Vec<_>>(), vec![Some(Dir::Back), Some(Dir::Back), Some(Dir::Fwd), Some(Dir::Fwd)]);
    assert!(lanes[0].left < lanes[2].left, "south lanes are on the left (west) when facing north");

    // Path drawn south: the same road, read from the other end. The north lanes are now on the left and run against the path.
    let south = layout(&p, &lib, [0.0, -1.0], Anchor::Center).unwrap();
    let lanes: Vec<_> = south.iter().filter(|e| e.id == "lane").collect();
    assert_eq!(lanes.iter().map(|l| l.dir).collect::<Vec<_>>(), vec![Some(Dir::Back), Some(Dir::Back), Some(Dir::Fwd), Some(Dir::Fwd)]);
    assert_eq!(south[0].id, "bike-lane");
    assert!((south.last().unwrap().right - 10.8).abs() < 1e-9);
}

#[test]
fn a_non_directional_element_ignores_a_direction() {
    let lib = Library::builtin();
    let placed = layout(&parse("margin N 3", &lib).unwrap(), &lib, [0.0, 1.0], Anchor::Left).unwrap();
    assert_eq!(placed[0].dir, None);
    assert_eq!((placed[0].left, placed[0].right), (0.0, 3.0));
}

#[test]
fn the_library_can_be_extended_and_overridden() {
    let mut lib = Library::builtin();
    lib.merge(&serde_json::json!({"elements": {"lane": {"role": "travel", "width": 3.3, "directional": true}, "tram-track": {"role": "travel", "width": 2.6}}})).unwrap();
    assert_eq!(lib.get("lane").unwrap().width, 3.3);
    assert!(parse("lane | tram-track", &lib).is_ok());
    assert!(lib.merge(&serde_json::json!({"elements": {"bad": {"role": "wibble", "width": 1.0}}})).is_err());
    assert!(lib.merge(&serde_json::json!({"elements": {"bad": {"role": "travel", "width": -1.0}}})).is_err());
}
