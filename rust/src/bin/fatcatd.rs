#![allow(missing_docs)]

extern crate chrono;
extern crate clap;
extern crate diesel;
//extern crate dotenv;
extern crate error_chain;
extern crate fatcat;
extern crate fatcat_api_spec;
extern crate futures;
extern crate iron;
extern crate iron_slog;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;

use clap::{App, Arg};
use iron::modifiers::RedirectRaw;
use iron::{status, Chain, Iron, IronResult, Request, Response};
use iron_slog::{DefaultLogFormatter, LoggerMiddleware};
use slog::{Drain, Logger};

/// Create custom server, wire it to the autogenerated router,
/// and pass it to the web server.
fn main() {
    let matches = App::new("server")
        .arg(
            Arg::with_name("https")
                .long("https")
                .help("Whether to use HTTPS or not"),
        )
        .get_matches();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = Logger::root(drain, o!());
    let formatter = DefaultLogFormatter;

    let server = fatcat::server().unwrap();
    let mut router = fatcat_api_spec::router(server);

    router.get("/", root_handler, "root-redirect");
    router.get("/swagger-ui", swaggerui_handler, "swagger-ui-html");
    router.get("/v0/openapi2.yml", yaml_handler, "openapi2-spec-yaml");

    fn root_handler(_: &mut Request) -> IronResult<Response> {
        //Ok(Response::with((status::Found, Redirect(Url::parse("/swagger-ui").unwrap()))))
        Ok(Response::with((
            status::Found,
            RedirectRaw("/swagger-ui".to_string()),
        )))
    }
    fn swaggerui_handler(_: &mut Request) -> IronResult<Response> {
        let html_type = "text/html".parse::<iron::mime::Mime>().unwrap();
        Ok(Response::with((
            html_type,
            status::Ok,
            include_str!("../../swagger-ui/index.html"),
        )))
    }
    fn yaml_handler(_: &mut Request) -> IronResult<Response> {
        Ok(Response::with((
            status::Ok,
            include_str!("../../../fatcat-openapi2.yml"),
        )))
    }

    let host_port = "localhost:9411";
    info!(
        logger,
        "Starting fatcatd API server on http://{}", &host_port
    );

    let mut chain = Chain::new(LoggerMiddleware::new(router, logger, formatter));

    // authentication
    chain.link_before(fatcat_api_spec::server::ExtractAuthData);
    chain.link_before(fatcat::auth::MacaroonAuthMiddleware::new());

    chain.link_after(fatcat::XClacksOverheadMiddleware);

    if matches.is_present("https") {
        unimplemented!()
    } else {
        // Using HTTP
        Iron::new(chain)
            .http(host_port)
            .expect("Failed to start HTTP server");
    }
}
