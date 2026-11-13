//! Dimensions taken from published standards, in metres, so the drawing code names them instead of scattering numbers.
//! Each says where it comes from (the entry id in `library/standards.json`, which gives the section and the quote). `core/tests/standards.rs`
//! checks every one against the limits in that file.

/// Stop line stroke: 12 to 24 in (`stop-line.width`).
pub const STOP_LINE_WIDTH: f64 = 0.6;
/// A stop line is at least 4 ft (1.22 m) in advance of the nearest crosswalk line (`stop-line.setback`).
pub const STOP_LINE_SETBACK_MIN: f64 = 1.22;
/// Longitudinal crosswalk bar: 12 to 24 in wide, spaced 12 to 60 in apart and at most 2.5 bar widths (`crosswalk.longitudinal-bars`).
pub const CROSSWALK_BAR_WIDTH: f64 = 0.5;
/// Centre-to-centre pitch of the bars (so the space between is 0.7 m).
pub const CROSSWALK_BAR_PITCH: f64 = 1.2;
/// Depth of a marked crosswalk along traffic: at least 6 ft (`crosswalk.min-width`).
pub const CROSSWALK_DEPTH: f64 = 3.0;
/// Yield line before a mid-block crosswalk across an uncontrolled approach: 20 to 50 ft (`stop-line.uncontrolled-multilane`).
pub const MIDBLOCK_YIELD_SETBACK: f64 = 6.2;
/// Crosswalk at a roundabout: at least 20 ft from the edge of the circulatory roadway (`crosswalk.roundabout-setback`).
pub const ROUNDABOUT_CROSSWALK_SETBACK: f64 = 6.2;
/// STOP sign circumradius: 30 in on a single-lane conventional road, 36 in on a multi-lane one (`sign.stop-size`).
pub const STOP_SIGN_RADIUS_SINGLE: f64 = 0.413;
pub const STOP_SIGN_RADIUS_MULTI: f64 = 0.495;
/// Dotted lane line: 3 ft segments, 9 ft gaps (`line.dotted-lane`).
pub const DOTTED_LANE_ON: f64 = 0.914;
pub const DOTTED_LANE_OFF: f64 = 2.743;
/// Yield line triangles: base 12 to 24 in, height 1.5 x the base, 3 to 12 in apart (`yield-line.triangles`).
pub const YIELD_TRIANGLE_BASE: f64 = 0.45;
pub const YIELD_TRIANGLE_HEIGHT: f64 = 0.675;
pub const YIELD_TRIANGLE_SPACE: f64 = 0.15;
/// The thinnest marking line the plug-in draws: a normal line is 4 to 6 in (`line.normal-width`).
pub const MARKING_LINE_MIN: f64 = 0.11;
/// Railroad: stop line at least 15 ft from the nearest rail (`rail.stop-line`).
pub const RAIL_STOP_LINE_MIN: f64 = 4.58;
/// Signal faces: primary faces at least 40 ft beyond the stop line (`signal.position`). Not yet applied: the drawn heads stand at the stop line.
pub const SIGNAL_MIN_BEYOND_STOP_LINE: f64 = 12.2;
