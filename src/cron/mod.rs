mod fetch_profiles;
use crate::state::AppState;
use crate::utils::cron::build_cron_expr;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tokio_cron_scheduler::{Job, JobScheduler};
pub async fn start_cron_jobs(
    state: Arc<AppState>,
) -> Result<JobScheduler, Box<dyn std::error::Error + Send + Sync>> {
    let scheduler = JobScheduler::new().await?;

    {
        let state = state.clone();
        tokio::spawn(async move {
            tracing::info!("🚀 Server restarted, waiting 5 seconds before first fetch_profiles...");
            sleep(Duration::from_secs(5)).await;

            tracing::info!("👤 Running initial fetch_profiles...");
            fetch_profiles::run(state).await;
        });
    }

    let (profiles_desc, profiles_cron_expr) =
        build_cron_expr(state.config.cron.fetch_profiles.seconds);

    tracing::info!(
        "📅 Scheduling fetch_profiles cron: {} → {}",
        profiles_desc,
        profiles_cron_expr
    );

    scheduler
        .add(
            Job::new_async(&profiles_cron_expr, {
                let state = state.clone();
                move |_uuid, _l| {
                    let state = state.clone();
                    Box::pin(async move {
                        fetch_profiles::run(state).await;
                    })
                }
            })
            .unwrap(),
        )
        .await
        .unwrap();
    scheduler.start().await?;

    Ok(scheduler)
}
