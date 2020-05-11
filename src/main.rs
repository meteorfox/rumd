use std::env;

use warp::Filter;

#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=rumd=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "rumd=info");
    }
    pretty_env_logger::init();

    // TODO(meteorfox): Asynchronously scan directories recursively for *.iso files
    // TODO(meteorfox): Validate that they are actually valid PSP ISO files
    // TODO(meteorfox): Build KV "database" of flat ISO filenames map to their entry
    // TODO(meteorfox): Each entry contains file length info and file-system path
    // TODO(meteorfox): When reading a range of bytes, look up in KV database, check
    //                  range within limits, open file and read bytes, close file then
    //                  return bytes.
    // TODO(meteorfox): Keep cache of chunks in memory, if necessary.

    let server_header = warp::reply::with::default_header("Server", "rumd v0.1.0");

    let api = filters::rumd();

    let routes = api.with(warp::log("rumd")).with(&server_header);

    warp::serve(routes).run(([10, 0, 0, 184], 41041)).await;
}

mod filters {
    use std::num::ParseIntError;
    use std::str::FromStr;
    use warp::{http::Response, http::StatusCode, hyper, Filter};

    pub fn rumd() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        umd_list().or(umd_info()).or(umd_read())
    }

    /// GET /
    pub fn umd_list() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let isos = vec![
            "/",
            "/Crisis%20Core%20-%20Final%20Fantasy%20VII%20(USA).iso",
            "Metal_Gear_Solid_Peace_Walker_USA_PSP-pSyPSP.iso",
            "Monster%20Hunter%20Freedom%20Unite%20(USA)%20(En,Fr,De,Es,It).iso",
        ];

        warp::path::end()
            .and(warp::get())
            .map(move || isos.join("\n"))
    }

    /// HEAD /<umd_name:string>
    pub fn umd_info() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path::param()
            .and(warp::path::end())
            .and(warp::head())
            .map(|umd_name: String| {
                log::debug!("info UMD: path={}", umd_name);

                Response::builder()
                    .header("Accept-Ranges", "bytes")
                    .header("Content-Type", "application/octet-stream")
                    .header("Content-Length", "1646002176")
                    .body(hyper::Body::empty())
            })
    }

    /// GET /<umd_name:string>
    pub fn umd_read() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let range_header = warp::header::<Range>("range");
        warp::path::param()
            .and(warp::path::end())
            .and(warp::get())
            .and(range_header)
            .map(|umd_name: String, range: Range| {
                log::debug!(
                    "Read UMD: path={} range=bytes {}-{}",
                    umd_name,
                    range.start,
                    range.end
                );

                Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header("Accept-Ranges", "bytes")
                    .header("Content-Type", "application/octet-stream")
                    .header(
                        "Content-Range",
                        format!("bytes {}-{}/{}", range.start, range.end, 1646002176),
                    )
                    .body(hyper::Body::empty())
            })
    }

    #[derive(Debug, PartialEq)]
    struct Range {
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
