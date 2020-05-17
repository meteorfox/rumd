use std::env;
use std::path::Path;

use ignore::types::TypesBuilder;
use ignore::WalkBuilder;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use warp::Filter;

const ROOT_ISO_PATH: &str = "/mnt/storage/games/psp";
const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ');

const RUMD_LISTENING_PORT: u16 = 41041;
const RUMD_VERSION: &str = "rumd v0.1.0";

#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=rumd=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "rumd=info");
    }
    pretty_env_logger::init();

    let mut builder = TypesBuilder::new();
    builder.add("iso", "*.iso").unwrap();
    builder.select("iso");
    let matcher = builder.build().unwrap();

    for result in WalkBuilder::new(ROOT_ISO_PATH).types(matcher).build() {
        match result {
            Ok(entry) => {
                let path: &Path = entry.path().strip_prefix(ROOT_ISO_PATH).unwrap();
                println!("/{}", utf8_percent_encode(path.to_str().unwrap(), FRAGMENT));
            }
            Err(err) => println!("ERROR: {}", err),
        }
    }

    // Must Have
    // TODO(meteorfox): Build KV "database" of flat ISO filenames map to their entry
    // TODO(meteorfox): Each entry contains file length info and file-system path
    // TODO(meteorfox): When reading a range of bytes, look up in KV database, check
    //                  range within limits, open file and read bytes, close file then
    //                  return bytes.

    // Nice to Have
    // TODO(meteorfox): Validate that they are actually valid PSP ISO files
    // TODO(meteorfox): Keep cache of blocks in memory, if necessary.

    let server_header = warp::reply::with::default_header("Server", RUMD_VERSION);

    let api = filters::rumd();

    let routes = api.with(warp::log("rumd")).with(&server_header);

    warp::serve(routes)
        .run(([10, 0, 0, 184], RUMD_LISTENING_PORT))
        .await;
}

mod filters {
    use super::rumd;

    use warp::Filter;

    pub fn rumd() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        umd_list().or(umd_info()).or(umd_read())
    }

    /// GET /
    pub fn umd_list() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path::end().and(warp::get()).and_then(rumd::list_umds)
    }

    /// HEAD /<umd_name:string>
    pub fn umd_info() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path::param()
            .and(warp::head())
            .and_then(rumd::info_umd)
    }

    /// GET /<umd_name:string>
    pub fn umd_read() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let range_header = warp::header::<rumd::Range>("range");
        warp::path::param()
            .and(warp::get())
            .and(range_header)
            .and_then(rumd::read_umd)
    }
}

mod rumd {
    use std::num::ParseIntError;
    use std::str::FromStr;

    use warp::http::{Response, StatusCode};
    use warp::hyper;

    pub async fn list_umds() -> Result<impl warp::Reply, warp::Rejection> {
        let isos = vec![
            "/",
            "/Crisis%20Core%20-%20Final%20Fantasy%20VII%20(USA).iso",
            "/Metal_Gear_Solid_Peace_Walker_USA_PSP-pSyPSP.iso",
            "/Monster%20Hunter%20Freedom%20Unite%20(USA)%20(En,Fr,De,Es,It).iso",
        ];
        Ok(isos.join("\n"))
    }

    pub async fn info_umd(umd_name: String) -> Result<impl warp::Reply, warp::Rejection> {
        log::debug!("info UMD: path={}", umd_name);

        let resp = Response::builder()
            .header("Accept-Ranges", "bytes")
            .header("Content-Type", "application/octet-stream")
            .header("Content-Length", "1646002176")
            .body(hyper::Body::empty());
        Ok(resp)
    }

    pub async fn read_umd(
        umd_name: String,
        range: Range,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        log::debug!(
            "Read UMD: path={} range=bytes {}-{}",
            umd_name,
            range.start,
            range.end
        );

        let resp = Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header("Accept-Ranges", "bytes")
            .header("Content-Type", "application/octet-stream")
            .header(
                "Content-Range",
                format!("bytes {}-{}/{}", range.start, range.end, 1646002176),
            )
            .body(hyper::Body::empty());

        Ok(resp)
    }

    #[derive(Debug, PartialEq)]
    pub struct Range {
        start: i64,
        end: i64,
    }

    impl FromStr for Range {
        type Err = ParseIntError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let range: Vec<&str> = s.trim_start_matches("bytes=").split('-').collect();
            let start_from_str = range[0].parse::<i64>()?;
            let end_from_str = range[1].parse::<i64>()?;
            Ok(Range {
                start: start_from_str,
                end: end_from_str,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::filters;

    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_list() {
        let api = filters::rumd();
        let resp = request().method("GET").path("/").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.body(),
            r#"/
/Crisis%20Core%20-%20Final%20Fantasy%20VII%20(USA).iso
/Metal_Gear_Solid_Peace_Walker_USA_PSP-pSyPSP.iso
/Monster%20Hunter%20Freedom%20Unite%20(USA)%20(En,Fr,De,Es,It).iso"#
        );
    }
}
