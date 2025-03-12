use anyhow::Result;
use turbo_tasks::{fxindexmap, Vc};
use turbo_tasks_env::{CommandLineProcessEnv, CustomProcessEnv, ProcessEnv};

#[turbo_tasks::function]
pub async fn load_env() -> Result<Vc<Box<dyn ProcessEnv>>> {
    let env: Vc<Box<dyn ProcessEnv>> = Vc::upcast(CommandLineProcessEnv::new());
    let node_env = env.read("NODE_ENV".into()).await?;
    let node_env = node_env.as_deref().unwrap_or("development");

    let env = Vc::upcast(CustomProcessEnv::new(
        env,
        Vc::cell(fxindexmap! {
            "NODE_ENV".into() => node_env.into()
        }),
    ));

    Ok(env)
}
