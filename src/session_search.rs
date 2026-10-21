//! Spindle: search the whole Spotify catalogue the way Spotify's own apps
//! do, through Pathfinder's `searchDesktop` query with the playback
//! session's Login5 token. The Web API search Spindle used before answers
//! outside apps with a handful of results, and none of Spotify's own
//! playlists; this one answers like the desktop app.

use librespot_core::Session;
use serde_json::Value;

use crate::api::models::{
    Album, Artist, ArtistRef, Episode, Image, Owner, Page, Playlist, SearchResults, Show, Track,
};

/// `searchDesktop` in Spotify's web player build of 2026-10-21. Spotify
/// changes it when it changes the query; a stale one answers
/// `PersistedQueryNotFound` and search falls back to the Web API.
const SEARCH_DESKTOP_HASH: &str =
    "eef7cc54888d91bdd6802623477873caa3948ae173a0c34fd86827b267e94c03";
/// Results per kind, as the web player asks for a full search page.
const LIMIT: u32 = 50;

/// Spindle: the playback grant's refresh token, saved when playback is
/// approved in the browser. Spotify answered the search for a session
/// started from that approval and refused it ("Client/request not
/// allowed") for the same session restored from its saved login, so the
/// search asks with an access token from this grant first. Kept in a
/// private file in the state folder and removed with the playback grant.
const GRANT_FILE: &str = "search-grant.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedGrant {
    refresh_token: String,
}

/// The access token minted from the saved grant, and when it expires.
static ACCESS: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);

pub fn remember_grant(state: &std::path::Path, refresh_token: &str) {
    let saved = SavedGrant {
        refresh_token: refresh_token.to_string(),
    };
    if let Ok(bytes) = serde_json::to_vec(&saved)
        && let Err(error) = fastframe_file::write_private(&state.join(GRANT_FILE), bytes)
    {
        log::warn!("could not keep the search grant: {error}");
    }
    if let Ok(mut access) = ACCESS.lock() {
        *access = None;
    }
}

/// Whether a browser approval for the catalogue search is saved.
pub fn has_grant(state: &std::path::Path) -> bool {
    state.join(GRANT_FILE).is_file()
}

pub fn forget_grant(state: &std::path::Path) {
    let _ = std::fs::remove_file(state.join(GRANT_FILE));
    if let Ok(mut access) = ACCESS.lock() {
        *access = None;
    }
}

/// An access token from the saved grant, refreshed when it is near expiry;
/// `None` without a grant or when Spotify will not refresh it.
async fn grant_token(state: &std::path::Path, http: &reqwest::Client) -> Option<String> {
    if let Ok(access) = ACCESS.lock()
        && let Some((token, until)) = access.as_ref()
        && *until > std::time::Instant::now()
    {
        return Some(token.clone());
    }
    let bytes = std::fs::read(state.join(GRANT_FILE)).ok()?;
    let saved: SavedGrant = serde_json::from_slice(&bytes).ok()?;
    match crate::auth::refresh(http, crate::auth::PLAYBACK_CLIENT_ID, &saved.refresh_token).await {
        Ok(response) => {
            if let Some(rotated) = response
                .refresh_token
                .as_deref()
                .filter(|token| !token.is_empty())
                && rotated != saved.refresh_token
            {
                remember_grant(state, rotated);
            }
            let lifetime = response.expires_in.unwrap_or(3600).saturating_sub(120);
            if let Ok(mut access) = ACCESS.lock() {
                *access = Some((
                    response.access_token.clone(),
                    std::time::Instant::now() + std::time::Duration::from_secs(lifetime),
                ));
            }
            Some(response.access_token)
        }
        Err(crate::auth::TokenEndpointError::Rejected { detail, .. }) => {
            log::warn!("Spotify would not refresh the search grant: {detail}");
            forget_grant(state);
            None
        }
        Err(error) => {
            log::warn!("search grant refresh failed: {error}");
            None
        }
    }
}

pub async fn search(
    session: &Session,
    web: &crate::api::ApiClient,
    state: &std::path::Path,
    query: &str,
) -> anyhow::Result<SearchResults> {
    // After every form was refused, Spotify gets ten minutes before it is
    // asked again, so each search does not pay for six refusals first.
    static REFUSED_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    if REFUSED_AT
        .lock()
        .ok()
        .and_then(|at| *at)
        .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(600))
    {
        anyhow::bail!("Spotify refused the full search a moment ago");
    }
    let client_token = session.spclient().client_token().await?;
    let mut bearers: Vec<(&str, String)> = Vec::new();
    if let Ok(http) = web.http_client()
        && let Some(token) = grant_token(state, &http).await
    {
        bearers.push(("the playback grant", token));
    }
    bearers.push((
        "the session",
        session.login5().auth_token().await?.access_token,
    ));
    // The web player's variables first; then the smaller set other clients
    // send, in case Spotify refuses a flag this account's build lacks.
    let full = serde_json::json!({
        "searchTerm": query,
        "offset": 0,
        "limit": LIMIT,
        "numberOfTopResults": 5,
        "includeAudiobooks": false,
        "includeArtistHasConcertsField": false,
        "includePreReleases": true,
        "includeAlbumPreReleases": false,
        "includeAuthors": false,
        "includeEpisodeContentRatingsV2": false,
        "isPrefix": null,
        "sectionFilters": ["GENERIC"]
    });
    let small = serde_json::json!({
        "searchTerm": query,
        "offset": 0,
        "limit": LIMIT,
        "numberOfTopResults": 5,
        "includeAudiobooks": true,
        "includeArtistHasConcertsField": false,
        "includePreReleases": true,
        "includeLocalConcertsField": false
    });
    // Spindle: Spotify accepted the plain request on one playback
    // connection and refused it ("Client/request not allowed", 403) on the
    // next. The forms below are tried in turn, the first that answers is
    // kept for the rest of the run, and each refusal is logged.
    static WORKING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    const FORMS: [(&str, bool, bool); 3] = [
        ("plain POST", false, false),
        ("web player headers", true, false),
        ("GET v1", true, true),
    ];
    let start = WORKING.load(std::sync::atomic::Ordering::Relaxed);
    let mut body = None;
    let mut last_error = None;
    'forms: for (bearer_name, bearer) in &bearers {
        for step in 0..FORMS.len() {
            let index = (start + step) % FORMS.len();
            let (name, web_headers, get) = FORMS[index];
            for variables in [&full, &small] {
                let request = serde_json::json!({
                    "operationName": "searchDesktop",
                    "variables": variables,
                    "extensions": {
                        "persistedQuery": {"version": 1, "sha256Hash": SEARCH_DESKTOP_HASH}
                    }
                });
                match web
                    .partner_request_as(bearer, &client_token, &request, web_headers, get)
                    .await
                {
                    Ok(answer) if answer.pointer("/data/searchV2").is_some() => {
                        log::info!("session search answered as {name} with {bearer_name}");
                        WORKING.store(index, std::sync::atomic::Ordering::Relaxed);
                        body = Some(answer);
                        break 'forms;
                    }
                    Ok(answer) => {
                        let message = answer
                            .pointer("/errors/0/message")
                            .and_then(Value::as_str)
                            .unwrap_or("no results")
                            .to_string();
                        log::warn!(
                            "session search as {name} with {bearer_name} answered: {message}"
                        );
                        last_error = Some(anyhow::anyhow!("Spotify's search answered: {message}"));
                    }
                    Err(error) => {
                        log::warn!("session search as {name} with {bearer_name} refused: {error}");
                        last_error = Some(anyhow::anyhow!("{error}"));
                    }
                }
            }
        }
    }
    let Some(body) = body else {
        if let Ok(mut at) = REFUSED_AT.lock() {
            *at = Some(std::time::Instant::now());
        }
        return Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Spotify's search failed")));
    };
    if let Some(error) = body.pointer("/errors/0/message").and_then(Value::as_str) {
        anyhow::bail!("Spotify's search answered: {error}");
    }
    let search = body
        .pointer("/data/searchV2")
        .ok_or_else(|| anyhow::anyhow!("Spotify answered without search results"))?;
    Ok(parse(search))
}

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn id_of(uri: &str) -> String {
    uri.rsplit(':').next().unwrap_or_default().to_string()
}

fn images(value: Option<&Value>) -> Vec<Image> {
    value
        .and_then(Value::as_array)
        .map(|sources| {
            sources
                .iter()
                .filter_map(|source| {
                    let url = source.get("url")?.as_str()?.to_string();
                    let size =
                        |key: &str| source.get(key).and_then(Value::as_u64).map(|n| n as u32);
                    Some(Image {
                        url,
                        width: size("width"),
                        height: size("height"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn artist_refs(value: &Value) -> Vec<ArtistRef> {
    value
        .pointer("/artists/items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|artist| {
                    let uri = text(artist, "/uri");
                    ArtistRef {
                        id: Some(id_of(&uri)).filter(|id| !id.is_empty()),
                        name: text(artist, "/profile/name"),
                        uri: Some(uri).filter(|uri| !uri.is_empty()),
                        ..Default::default()
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The rows of one section whose `data` is of `kind`, by `data`.
fn section<'a>(search: &'a Value, key: &str, kind: &str, wrapped: bool) -> (Vec<&'a Value>, u32) {
    let section = search.get(key);
    let total = section
        .and_then(|section| section.get("totalCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let rows = section
        .and_then(|section| section.get("items"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|row| {
                    let row = if wrapped { row.get("item")? } else { row };
                    let data = row.get("data")?;
                    (data.get("__typename").and_then(Value::as_str) == Some(kind)).then_some(data)
                })
                .collect()
        })
        .unwrap_or_default();
    (rows, total)
}

fn page<T>(items: Vec<T>, total: u32) -> Option<Page<T>> {
    let count = items.len() as u32;
    Some(Page {
        items,
        total: total.max(count),
        limit: count,
        offset: 0,
        next: None,
    })
}

fn track(data: &Value) -> Track {
    let uri = text(data, "/uri");
    let album_uri = text(data, "/albumOfTrack/uri");
    Track {
        id: Some(id_of(&uri)),
        name: text(data, "/name"),
        duration_ms: data
            .pointer("/duration/totalMilliseconds")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        explicit: data.pointer("/contentRating/label").and_then(Value::as_str) == Some("EXPLICIT"),
        artists: artist_refs(data),
        album: Some(Album {
            id: id_of(&album_uri),
            name: text(data, "/albumOfTrack/name"),
            uri: album_uri,
            images: images(data.pointer("/albumOfTrack/coverArt/sources")),
            ..Default::default()
        }),
        is_playable: data
            .pointer("/playability/playable")
            .and_then(Value::as_bool),
        uri,
        ..Default::default()
    }
}

fn parse(search: &Value) -> SearchResults {
    let (rows, total) = section(search, "tracksV2", "Track", true);
    let tracks = page(rows.into_iter().map(track).collect(), total);

    let (rows, total) = section(search, "artists", "Artist", false);
    let artists = page(
        rows.into_iter()
            .map(|data| {
                let uri = text(data, "/uri");
                Artist {
                    id: id_of(&uri),
                    name: text(data, "/profile/name"),
                    images: images(data.pointer("/visuals/avatarImage/sources")),
                    uri,
                    ..Default::default()
                }
            })
            .collect(),
        total,
    );

    let (rows, total) = section(search, "albumsV2", "Album", false);
    let albums = page(
        rows.into_iter()
            .map(|data| {
                let uri = text(data, "/uri");
                let kind = text(data, "/type").to_ascii_lowercase();
                Album {
                    id: id_of(&uri),
                    name: text(data, "/name"),
                    images: images(data.pointer("/coverArt/sources")),
                    artists: artist_refs(data),
                    release_date: data
                        .pointer("/date/year")
                        .and_then(Value::as_u64)
                        .map(|year| year.to_string()),
                    album_type: Some(if kind == "ep" { "single".into() } else { kind })
                        .filter(|kind| !kind.is_empty()),
                    uri,
                    ..Default::default()
                }
            })
            .collect(),
        total,
    );

    let (rows, total) = section(search, "playlists", "Playlist", false);
    let playlists = page(
        rows.into_iter()
            .map(|data| {
                let uri = text(data, "/uri");
                let owner_uri = text(data, "/ownerV2/data/uri");
                let pictures = data
                    .pointer("/images/items")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .flat_map(|item| images(item.get("sources")))
                            .collect()
                    })
                    .unwrap_or_default();
                Playlist {
                    id: id_of(&uri),
                    name: text(data, "/name"),
                    description: Some(text(data, "/description")).filter(|d| !d.is_empty()),
                    images: pictures,
                    owner: Owner {
                        id: Some(text(data, "/ownerV2/data/username"))
                            .filter(|id| !id.is_empty())
                            .or_else(|| Some(id_of(&owner_uri)).filter(|id| !id.is_empty())),
                        display_name: Some(text(data, "/ownerV2/data/name"))
                            .filter(|name| !name.is_empty()),
                        uri: Some(owner_uri).filter(|uri| !uri.is_empty()),
                    },
                    uri,
                    ..Default::default()
                }
            })
            .collect(),
        total,
    );

    let (rows, total) = section(search, "podcasts", "Podcast", false);
    let shows = page(
        rows.into_iter()
            .map(|data| {
                let uri = text(data, "/uri");
                Show {
                    id: id_of(&uri),
                    name: text(data, "/name"),
                    publisher: text(data, "/publisher/name"),
                    images: images(data.pointer("/coverArt/sources")),
                    uri,
                    ..Default::default()
                }
            })
            .collect(),
        total,
    );

    let (rows, total) = section(search, "episodes", "Episode", false);
    let episodes = page(
        rows.into_iter()
            .map(|data| {
                let uri = text(data, "/uri");
                let show_uri = text(data, "/podcastV2/data/uri");
                Episode {
                    id: id_of(&uri),
                    name: text(data, "/name"),
                    description: text(data, "/description"),
                    images: images(data.pointer("/coverArt/sources")),
                    duration_ms: data
                        .pointer("/duration/totalMilliseconds")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                    release_date: data
                        .pointer("/releaseDate/isoString")
                        .and_then(Value::as_str)
                        .map(|date| date.get(..10).unwrap_or(date).to_string()),
                    explicit: data.pointer("/contentRating/label").and_then(Value::as_str)
                        == Some("EXPLICIT"),
                    show: Some(Show {
                        id: id_of(&show_uri),
                        name: text(data, "/podcastV2/data/name"),
                        publisher: text(data, "/podcastV2/data/publisher/name"),
                        images: images(data.pointer("/podcastV2/data/coverArt/sources")),
                        uri: show_uri,
                        ..Default::default()
                    })
                    .filter(|show| !show.uri.is_empty()),
                    uri,
                    ..Default::default()
                }
            })
            .collect(),
        total,
    );

    SearchResults {
        tracks,
        artists,
        albums,
        playlists,
        shows,
        episodes,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_desktop_search_answer_fills_every_section() {
        let answer = serde_json::json!({
            "tracksV2": {"totalCount": 900, "items": [
                {"item": {"__typename": "TrackResponseWrapper", "data": {
                    "__typename": "Track", "uri": "spotify:track:t1", "name": "Blessed",
                    "duration": {"totalMilliseconds": 210000},
                    "contentRating": {"label": "EXPLICIT"},
                    "playability": {"playable": true},
                    "albumOfTrack": {"uri": "spotify:album:a1", "name": "Freudian",
                        "coverArt": {"sources": [{"url": "https://i/a.jpg", "width": 300, "height": 300}]}},
                    "artists": {"items": [{"uri": "spotify:artist:r1", "profile": {"name": "Daniel Caesar"}}]}
                }}},
                {"item": {"__typename": "UnknownTypeWrapper", "data": {"__typename": "Other"}}}
            ]},
            "artists": {"totalCount": 3, "items": [
                {"__typename": "ArtistResponseWrapper", "data": {"__typename": "Artist",
                    "uri": "spotify:artist:r1", "profile": {"name": "Daniel Caesar"},
                    "visuals": {"avatarImage": {"sources": [{"url": "https://i/r.jpg"}]}}}}
            ]},
            "albumsV2": {"totalCount": 2, "items": [
                {"__typename": "AlbumResponseWrapper", "data": {"__typename": "Album",
                    "uri": "spotify:album:a1", "name": "Freudian", "type": "ALBUM",
                    "date": {"year": 2017}, "coverArt": {"sources": []},
                    "artists": {"items": [{"uri": "spotify:artist:r1", "profile": {"name": "Daniel Caesar"}}]}}}
            ]},
            "playlists": {"totalCount": 1000, "items": [
                {"__typename": "PlaylistResponseWrapper", "data": {"__typename": "Playlist",
                    "uri": "spotify:playlist:p1", "name": "This Is Daniel Caesar",
                    "images": {"items": [{"sources": [{"url": "https://i/p.jpg"}]}]},
                    "ownerV2": {"data": {"__typename": "User", "uri": "spotify:user:spotify",
                        "name": "Spotify", "username": "spotify"}}}}
            ]},
            "podcasts": {"totalCount": 0, "items": []},
            "episodes": {"totalCount": 0, "items": []}
        });
        let results = parse(&answer);
        let tracks = results.tracks.unwrap();
        assert_eq!(tracks.total, 900);
        assert_eq!(tracks.items.len(), 1);
        let track = &tracks.items[0];
        assert_eq!(track.uri, "spotify:track:t1");
        assert_eq!(track.duration_ms, 210000);
        assert!(track.explicit);
        assert_eq!(track.artists[0].name, "Daniel Caesar");
        assert_eq!(track.album.as_ref().unwrap().images[0].width, Some(300));
        assert_eq!(
            results.artists.unwrap().items[0].images[0].url,
            "https://i/r.jpg"
        );
        let album = &results.albums.unwrap().items[0];
        assert_eq!(
            (album.year(), album.album_type.as_deref()),
            (Some("2017"), Some("album"))
        );
        let playlist = &results.playlists.unwrap().items[0];
        assert_eq!(playlist.name, "This Is Daniel Caesar");
        assert_eq!(playlist.owner.id.as_deref(), Some("spotify"));
        assert_eq!(playlist.owner.display_name.as_deref(), Some("Spotify"));
        assert_eq!(playlist.images[0].url, "https://i/p.jpg");
        assert!(results.shows.unwrap().items.is_empty());
    }
}
