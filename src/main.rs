use slack_morphism::prelude::*;
use slack_morphism_hyper::*;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use log::*;

use std::sync::Arc;

type QLLError = Box<dyn std::error::Error + Sync + Send>;
type Result<T> = std::result::Result<T, QLLError>;

async fn send_message(client: Arc<SlackHyperClient>) -> Result<()> {
    let message = SlackMessageContent::new()
        .with_blocks(slack_blocks![
            some_into(SlackSectionBlock::new().with_text(md!("Queue leu leu"))),
            some_into(SlackDividerBlock::new())
        ]);

    Ok(())
}

async fn default_route(_req: Request<Body>) -> Result<Response<Body>> {
    Response::builder()
        .body("Hey, this is a default users route handler".into())
        .map_err(|e| e.into())
}

async fn oauth_install_function(
    resp: SlackOAuthV2AccessTokenResponse,
    _client: Arc<SlackHyperClient>,
    _states: Arc<SlackClientEventsUserState>,
) {
    println!("{:#?}", resp);
}

async fn interaction_events_function(
    event: SlackInteractionEvent,
    _client: Arc<SlackHyperClient>,
    _states: Arc<SlackClientEventsUserState>,
) -> Result<()> {
    println!("{:#?}", event);
    Ok(())
}

async fn command_events_function(
    event: SlackCommandEvent,
    client: Arc<SlackHyperClient>,
    _states: Arc<SlackClientEventsUserState>,
) -> Result<SlackCommandEventResponse> {
    let token_value = config_env_var("SLACK_QLL_TOKEN")?.into();
    let token = SlackApiToken::new(token_value);
    let session = client.open_session(&token);

    session
        .api_test(&SlackApiTestRequest::new().with_foo("Test".into()))
        .await?;

    println!("{:#?}", event);
    Ok(SlackCommandEventResponse::new(
        SlackMessageContent::new().with_text("Working on it".into()),
    ))
}

fn error_handler(
    err: QLLError,
    _client: Arc<SlackHyperClient>,
    _states: Arc<SlackClientEventsUserState>,
) -> http::StatusCode {
    println!("{:#?}", err);

    // Defines what we return Slack server
    http::StatusCode::BAD_REQUEST
}

async fn run_server() -> Result<()> {
    let client: Arc<SlackHyperClient> =
        Arc::new(SlackClient::new(SlackClientHyperConnector::new()));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("Loading server: {}", addr);

    let oauth_listener_config = Arc::new(SlackOAuthListenerConfig::new(
        config_env_var("SLACK_CLIENT_ID")?,
        config_env_var("SLACK_CLIENT_SECRET")?,
        config_env_var("SLACK_BOT_SCOPE")?,
        config_env_var("SLACK_REDIRECT_HOST")?,
    ));

    let interactions_events_config = Arc::new(SlackInteractionEventsListenerConfig::new(
        config_env_var("SLACK_SIGNING_SECRET")?,
    ));

    let command_events_config = Arc::new(SlackCommandEventsListenerConfig::new(config_env_var(
        "SLACK_SIGNING_SECRET",
    )?));

    let listener_environment = Arc::new(
        SlackClientEventsListenerEnvironment::new(client.clone())
            .with_error_handler(error_handler)
    );

    let make_service = make_service_fn(move |_| {
        let thread_oauth_config = oauth_listener_config.clone();
        let thread_interaction_events_config = interactions_events_config.clone();
        let thread_command_events_config = command_events_config.clone();
        let listener = SlackClientEventsHyperListener::new(listener_environment.clone());

        async move {
            let routes = chain_service_routes_fn(
                listener.oauth_service_fn(thread_oauth_config, oauth_install_function),
                chain_service_routes_fn(
                    listener.interaction_events_service_fn(
                        thread_interaction_events_config,
                        interaction_events_function,
                    ),
                    chain_service_routes_fn(
                        listener.command_events_service_fn(
                            thread_command_events_config,
                            command_events_function,
                        ),
                        default_route,
                    ),
                ),
            );

            Ok::<_, QLLError>(service_fn(routes))
        }
    });

    let server = hyper::server::Server::bind(&addr).serve(make_service);
    server.await.map_err(|e| {
        error!("Server error: {}", e);
        e.into()
    })
}

fn init_log() -> Result<()> {
    use fern::colors::{Color, ColoredLevelConfig};

    let colors_level = ColoredLevelConfig::new()
        .info(Color::Green)
        .warn(Color::Magenta);

    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}{}\x1B[0m",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                colors_level.color(record.level()),
                format_args!(
                    "\x1B[{}m",
                    colors_level.get_color(&record.level()).to_fg_str()
                ),
                message
            ))
        })
        // Add blanket level filter -
        .level(log::LevelFilter::Debug)
        // - and per-module overrides
        .level_for("slack_morphism", log::LevelFilter::Debug)
        .level_for("slack_morphism_hyper", log::LevelFilter::Debug)
        .level_for("hyper", log::LevelFilter::Info)
        .level_for("rustls", log::LevelFilter::Info)
        .level_for("hyper_rustls", log::LevelFilter::Info)
        // Output to stdout, files, and other Dispatch configurations
        .chain(std::io::stdout())
        // Apply globally
        .apply()?;

    Ok(())
}

pub fn config_env_var(name: &str) -> std::result::Result<String, String> {
    std::env::var(name).map_err(|e| format!("{}: {}", name, e))
}

#[tokio::main]
async fn main() -> Result<()> {
    init_log()?;

    run_server().await?;

    Ok(())
}
