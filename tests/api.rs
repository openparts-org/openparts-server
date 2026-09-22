//! Integration tests against the real `openparts-data` fixtures, driven
//! through the router directly (`tower::ServiceExt::oneshot`) rather
//! than a bound socket.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../openparts-data")
}

fn app() -> axum::Router {
    let store = openparts_server::store::discover_and_load(&data_dir()).unwrap();
    openparts_server::routes::build_router(Arc::new(store))
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn health_check_ok() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn search_finds_rp2040() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/search?q=RP2040")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("\"mpn\":\"RP2040\""));
}

#[tokio::test]
async fn get_part_returns_404_for_unknown_part() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/nobody/NOPE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_part_returns_rp2040() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("\"mpn\":\"RP2040\""));
}

#[tokio::test]
async fn kicad_symbol_artifact_matches_device_pin_count() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-content-hash"));
    let body = body_string(response).await;
    // 56 perimeter pins + EP.
    assert_eq!(body.matches("(number \"").count(), 57);
}

#[tokio::test]
async fn stl_artifact_has_one_solid_per_part() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/stl")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-content-hash"));
    let body = body_string(response).await;
    assert!(body.starts_with("solid RP2040\n"));
    // 1 body + 56 leads = 57 boxes, 12 facets each.
    assert_eq!(body.matches("facet normal").count(), 57 * 12);
}

#[tokio::test]
async fn revision_query_param_changes_effective_model() {
    // RP2040's real B0/B1/B2 revisions have no documented pin
    // differences (unlike the old fictional exemplar fixture) -- this
    // just proves the revision selection path is wired end to end, not
    // that it changes output for this specific device.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol?silicon_revision=rev-b2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_revision_is_rejected_with_422() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol?silicon_revision=does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn esp32_c6_footprint_now_generates_after_jedec_dimensions_were_added() {
    // ESP32-C6's package pitch/height weren't confirmable from its own
    // datasheet text (only an embedded mechanical drawing image); this
    // used to make footprint generation correctly fail with 422
    // (missing dimension) rather than guess. openparts-data later added
    // JEDEC MO-220 standard pitch/height for this exact lead-count/
    // body-size combination (see that repo's history), so generation
    // now succeeds -- this is a regression test for that fix, not a
    // live example of "missing dimension blocks generation" anymore.
    // That principle itself stays covered by openparts-mcad's own
    // McadError::MissingDimension unit tests.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/espressif/ESP32-C6/artifacts/kicad-footprint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert_eq!(body.matches("(pad \"").count(), 40);
}

#[tokio::test]
async fn chip_resistor_footprint_has_two_pads() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/parts/yageo/RC0603FR-0710KL/artifacts/kicad-footprint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert_eq!(body.matches("(pad \"").count(), 2);
}

#[tokio::test]
async fn get_device_resolves_slash_containing_id() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/devices/raspberrypi/RP2040")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
