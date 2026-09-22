//! Versioned REST API (Architecture Specification section 46-49).
//! Canonical Data editing is intentionally out of scope for this crate
//! (section 4.8 / section 20: "Canonical Data編集機能はServerの責務に
//!含めない") -- every route here is read-only.

use crate::error::ApiError;
use crate::store::Store;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use openparts_core::{Device, Package, Part, ProvenanceMap, SourceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::Arc;

pub fn build_router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/search", get(search))
        .route("/v1/parts/:manufacturer/:mpn", get(get_part))
        .route(
            "/v1/parts/:manufacturer/:mpn/provenance",
            get(get_provenance),
        )
        .route("/v1/parts/:manufacturer/:mpn/revisions", get(get_revisions))
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts",
            get(list_artifacts),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/kicad-symbol",
            get(artifact_kicad_symbol),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/kicad-footprint",
            get(artifact_kicad_footprint),
        )
        .route(
            "/v1/parts/:manufacturer/:mpn/artifacts/step",
            get(artifact_step),
        )
        .route("/v1/devices/*id", get(get_device))
        .route("/v1/packages/*id", get(get_package))
        .with_state(store)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Serialize)]
struct PartSummary {
    manufacturer: String,
    mpn: String,
    existence: openparts_core::ExistenceStatus,
    lifecycle: openparts_core::LifecycleStatus,
}

/// `GET /v1/search?q=...`
async fn search(
    State(store): State<Arc<Store>>,
    Query(params): Query<SearchQuery>,
) -> Json<Vec<PartSummary>> {
    let needle = params.q.unwrap_or_default().to_lowercase();
    let mut results: Vec<PartSummary> = store
        .parts
        .values()
        .filter(|p| {
            needle.is_empty()
                || p.mpn.to_lowercase().contains(&needle)
                || p.manufacturer.0.to_lowercase().contains(&needle)
        })
        .map(|p| PartSummary {
            manufacturer: p.manufacturer.0.clone(),
            mpn: p.mpn.clone(),
            existence: p.existence.status,
            lifecycle: p.lifecycle.status,
        })
        .collect();
    results.sort_by(|a, b| (&a.manufacturer, &a.mpn).cmp(&(&b.manufacturer, &b.mpn)));
    Json(results)
}

fn resolve_part<'s>(store: &'s Store, manufacturer: &str, mpn: &str) -> Result<&'s Part, ApiError> {
    store
        .parts
        .get(&(manufacturer.to_string(), mpn.to_string()))
        .ok_or_else(|| ApiError::NotFound(format!("no part {manufacturer}/{mpn}")))
}

fn resolve_device<'s>(store: &'s Store, id: &str) -> Result<&'s Device, ApiError> {
    store
        .devices
        .get(id)
        .ok_or_else(|| ApiError::NotFound(format!("no device {id}")))
}

fn resolve_package<'s>(store: &'s Store, id: &str) -> Result<&'s Package, ApiError> {
    store
        .packages
        .get(id)
        .ok_or_else(|| ApiError::NotFound(format!("no package {id}")))
}

/// `GET /v1/parts/{manufacturer}/{mpn}`
async fn get_part(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<Part>, ApiError> {
    Ok(Json(resolve_part(&store, &manufacturer, &mpn)?.clone()))
}

/// `GET /v1/devices/{id}` (`id` may itself contain `/`, e.g.
/// `raspberrypi/RP2040` -- captured as a wildcard tail segment).
async fn get_device(
    State(store): State<Arc<Store>>,
    Path(id): Path<String>,
) -> Result<Json<Device>, ApiError> {
    Ok(Json(resolve_device(&store, &id)?.clone()))
}

/// `GET /v1/packages/{id}`
async fn get_package(
    State(store): State<Arc<Store>>,
    Path(id): Path<String>,
) -> Result<Json<Package>, ApiError> {
    Ok(Json(resolve_package(&store, &id)?.clone()))
}

#[derive(Serialize)]
struct ProvenanceResponse {
    part: ProvenanceMap,
    device: ProvenanceMap,
    package: ProvenanceMap,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/provenance`
async fn get_provenance(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<ProvenanceResponse>, ApiError> {
    let part = resolve_part(&store, &manufacturer, &mpn)?;
    let device = resolve_device(&store, &part.device.0)?;
    let package = resolve_package(&store, &part.package.0)?;
    Ok(Json(ProvenanceResponse {
        part: part.provenance.clone(),
        device: device.provenance.clone(),
        package: package.provenance.clone(),
    }))
}

#[derive(Serialize)]
struct RevisionSummary {
    id: String,
    manufacturer_revision: String,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/revisions`
async fn get_revisions(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<Vec<RevisionSummary>>, ApiError> {
    let part = resolve_part(&store, &manufacturer, &mpn)?;
    let device = resolve_device(&store, &part.device.0)?;
    let mut revisions: Vec<RevisionSummary> = device
        .revisions
        .iter()
        .map(|(id, rev)| RevisionSummary {
            id: id.clone(),
            manufacturer_revision: rev.manufacturer_revision.clone(),
        })
        .collect();
    revisions.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(revisions))
}

#[derive(Serialize)]
struct ArtifactsResponse {
    #[serde(rename = "kicad-symbol")]
    kicad_symbol: String,
    #[serde(rename = "kicad-footprint")]
    kicad_footprint: String,
    step: String,
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts`
async fn list_artifacts(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
) -> Result<Json<ArtifactsResponse>, ApiError> {
    resolve_part(&store, &manufacturer, &mpn)?;
    let base = format!("/v1/parts/{manufacturer}/{mpn}/artifacts");
    Ok(Json(ArtifactsResponse {
        kicad_symbol: format!("{base}/kicad-symbol"),
        kicad_footprint: format!("{base}/kicad-footprint"),
        step: format!("{base}/step"),
    }))
}

#[derive(Deserialize)]
struct RevisionQuery {
    silicon_revision: Option<String>,
}

/// Loads and validates the Part/Device/Package triple, then builds the
/// Effective Model for the requested (or base) revision. Shared by all
/// three artifact endpoints (Architecture Specification section 51:
/// Request -> Part Resolution -> Effective Model -> Domain IR -> Format
/// Adapter -> Artifact).
fn build_model(
    store: &Store,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
) -> Result<openparts_core::EffectiveModel, ApiError> {
    let part = resolve_part(store, manufacturer, mpn)?.clone();
    let device = resolve_device(store, &part.device.0)?.clone();
    let package = resolve_package(store, &part.package.0)?.clone();

    let known_sources: BTreeSet<SourceId> = store
        .sources
        .keys()
        .map(|id| SourceId(id.clone()))
        .collect();
    let diags = openparts_validator::validate_part(&part, &device, &package, &known_sources);
    if !diags.is_empty() {
        return Err(ApiError::ValidationFailed(diags));
    }

    openparts_core::build_effective_model(part, &device, package, revision)
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))
}

fn artifact_response(content: String, content_type: &'static str) -> Response {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hex_encode(&hasher.finalize());

    let mut response = (StatusCode::OK, content).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type),
    );
    if let Ok(value) = header::HeaderValue::from_str(&format!("sha256:{hash}")) {
        response.headers_mut().insert("x-content-hash", value);
    }
    response
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-symbol?silicon_revision=...`
async fn artifact_kicad_symbol(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    let model = build_model(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    let symbol = openparts_pcbcad::build_symbol(&model.part.mpn, &model.device);
    let text = openparts_kicad::render_symbol(&symbol);
    Ok(artifact_response(text, "text/plain; charset=utf-8"))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-footprint?silicon_revision=...`
async fn artifact_kicad_footprint(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    let model = build_model(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    let geometry = openparts_mcad::generate(&model.package)
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
    let footprint = openparts_pcbcad::build_footprint(&model.part.mpn, &geometry);
    let text = openparts_kicad::render_footprint(&footprint);
    Ok(artifact_response(text, "text/plain; charset=utf-8"))
}

/// `GET /v1/parts/{manufacturer}/{mpn}/artifacts/step?silicon_revision=...`
async fn artifact_step(
    State(store): State<Arc<Store>>,
    Path((manufacturer, mpn)): Path<(String, String)>,
    Query(q): Query<RevisionQuery>,
) -> Result<Response, ApiError> {
    let model = build_model(&store, &manufacturer, &mpn, q.silicon_revision.as_deref())?;
    let geometry = openparts_mcad::generate(&model.package)
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
    let text = openparts_step::generate_step(&geometry, &model.part.mpn)
        .map_err(|e| ApiError::GenerationFailed(e.to_string()))?;
    Ok(artifact_response(text, "model/step"))
}
