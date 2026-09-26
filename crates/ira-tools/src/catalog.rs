use crate::args::{
    opt_bool, opt_f64, opt_i64, opt_str, opt_str_list, opt_u64, require_i64, require_str,
};
use crate::error::stringify;
use crate::registry::Builder;

pub fn register(b: &mut Builder) {
    files(b);
    shell(b);
    system(b);
    weather(b);
    appflowy(b);
    github(b);
    google(b);
    home_assistant(b);
    callmebot(b);
    db(b);
    kanban(b);
}

fn files(b: &mut Builder) {
    b.add_fn(ira_tools_files::read_file::spec(), |ctx, args| async move {
        let path = ctx.resolve(require_str(&args, "path")?);
        stringify(ira_tools_files::read_file::run(&path, opt_u64(&args, "max_bytes")).await)
    });
    b.add_fn(
        ira_tools_files::write_file::spec(),
        |ctx, args| async move {
            let path = ctx.resolve(require_str(&args, "path")?);
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            stringify(
                ira_tools_files::write_file::run(
                    &path,
                    content,
                    opt_bool(&args, "append").unwrap_or(false),
                )
                .await,
            )
        },
    );
    b.add_fn(
        ira_tools_files::list_directory::spec(),
        |ctx, args| async move {
            let path = ctx.resolve(opt_str(&args, "path").unwrap_or("."));
            stringify(
                ira_tools_files::list_directory::run(
                    &path,
                    opt_bool(&args, "recursive").unwrap_or(false),
                )
                .await,
            )
        },
    );
    b.add_fn(
        ira_tools_files::search_files::spec(),
        |ctx, args| async move {
            let query = require_str(&args, "query")?.to_string();
            let path = ctx.resolve(opt_str(&args, "path").unwrap_or("."));
            stringify(
                ira_tools_files::search_files::run(&path, &query, opt_u64(&args, "max_results"))
                    .await,
            )
        },
    );
    b.add_fn(ira_tools_files::move_file::spec(), |ctx, args| async move {
        let from = ctx.resolve(require_str(&args, "from")?);
        let to = ctx.resolve(require_str(&args, "to")?);
        stringify(ira_tools_files::move_file::run(&from, &to).await)
    });
    b.add_fn(ira_tools_files::copy_file::spec(), |ctx, args| async move {
        let from = ctx.resolve(require_str(&args, "from")?);
        let to = ctx.resolve(require_str(&args, "to")?);
        stringify(ira_tools_files::copy_file::run(&from, &to).await)
    });
    b.add_fn(
        ira_tools_files::remove_file::spec(),
        |ctx, args| async move {
            let path = ctx.resolve(require_str(&args, "path")?);
            stringify(
                ira_tools_files::remove_file::run(
                    &path,
                    opt_bool(&args, "recursive").unwrap_or(false),
                )
                .await,
            )
        },
    );
}

fn shell(b: &mut Builder) {
    b.add_fn(
        ira_tools_shell::execute_command::spec(),
        |ctx, args| async move {
            let program = opt_str(&args, "program").map(str::to_string);
            let command = opt_str(&args, "command").map(str::to_string);
            let cwd = opt_str(&args, "cwd").map(|p| ctx.resolve(p));
            let cwd_ref = cwd.as_deref().or(Some(ctx.cwd.as_path()));
            stringify(
                ira_tools_shell::execute_command::run(
                    program.as_deref(),
                    &opt_str_list(&args, "args"),
                    command.as_deref(),
                    cwd_ref,
                    opt_u64(&args, "timeout_secs"),
                )
                .await,
            )
        },
    );
    b.add_fn(
        ira_tools_shell::execute_script::spec(),
        |ctx, args| async move {
            let script = require_str(&args, "script")?.to_string();
            let interpreter = opt_str(&args, "interpreter").map(str::to_string);
            let cwd = opt_str(&args, "cwd").map(|p| ctx.resolve(p));
            let cwd_ref = cwd.as_deref().or(Some(ctx.cwd.as_path()));
            stringify(
                ira_tools_shell::execute_script::run(
                    &script,
                    interpreter.as_deref(),
                    cwd_ref,
                    opt_u64(&args, "timeout_secs"),
                )
                .await,
            )
        },
    );
}

fn system(b: &mut Builder) {
    b.add_fn(
        ira_tools_system::process::list_processes::spec(),
        |_ctx, args| async move {
            stringify(
                ira_tools_system::process::list_processes::run(
                    opt_str(&args, "query"),
                    opt_u64(&args, "limit"),
                )
                .await,
            )
        },
    );
    b.add_fn(
        ira_tools_system::process::get_process::spec(),
        |_ctx, args| async move {
            stringify(
                ira_tools_system::process::get_process::run(require_i64(&args, "pid")? as i32)
                    .await,
            )
        },
    );
    b.add_fn(
        ira_tools_system::process::kill_process::spec(),
        |_ctx, args| async move {
            stringify(
                ira_tools_system::process::kill_process::run(
                    require_i64(&args, "pid")? as i32,
                    opt_str(&args, "signal"),
                )
                .await,
            )
        },
    );
    b.add_fn(
        ira_tools_system::desktop::show_notification::spec(),
        |_ctx, args| async move {
            stringify(
                ira_tools_system::desktop::show_notification::run(
                    require_str(&args, "title")?,
                    opt_str(&args, "body"),
                )
                .await,
            )
        },
    );
    b.add_fn(ira_tools_system::desktop::read_clipboard::spec(), |_ctx, _args| async move {
        stringify(ira_tools_system::desktop::read_clipboard::run().await)
    });
    b.add_fn(
        ira_tools_system::desktop::write_clipboard::spec(),
        |_ctx, args| async move {
            stringify(
                ira_tools_system::desktop::write_clipboard::run(require_str(&args, "text")?).await,
            )
        },
    );
    b.add_fn(
        ira_tools_system::desktop::open_url::spec(),
        |_ctx, args| async move {
            stringify(ira_tools_system::desktop::open_url::run(require_str(&args, "url")?).await)
        },
    );
    b.add_fn(
        ira_tools_system::desktop::open_application::spec(),
        |_ctx, args| async move {
            stringify(
                ira_tools_system::desktop::open_application::run(require_str(&args, "name")?).await,
            )
        },
    );
}

fn weather(b: &mut Builder) {
    b.add_fn(
        ira_tools_weather::get_weather::spec(),
        |ctx, args| async move {
            stringify(
                ira_tools_weather::get_weather::run(
                    &ctx.http,
                    opt_str(&args, "place"),
                    opt_f64(&args, "latitude"),
                    opt_f64(&args, "longitude"),
                )
                .await,
            )
        },
    );
    b.add_fn(
        ira_tools_weather::get_forecast::spec(),
        |ctx, args| async move {
            stringify(
                ira_tools_weather::get_forecast::run(
                    &ctx.http,
                    opt_str(&args, "place"),
                    opt_f64(&args, "latitude"),
                    opt_f64(&args, "longitude"),
                    opt_u64(&args, "days"),
                )
                .await,
            )
        },
    );
}

fn appflowy(b: &mut Builder) {
    let Some(client) = ira_tools_appflowy::Client::from_env() else {
        return;
    };
    tracing::info!("tools appflowy listas");
    let c = client.clone();
    b.add_fn(
        ira_tools_appflowy::create_page::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_appflowy::create_page::run(
                        &c,
                        require_str(&args, "title")?,
                        opt_str(&args, "markdown").unwrap_or(""),
                        opt_str(&args, "parent_view_id"),
                    )
                    .await,
                )
            }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_appflowy::create_page::spec_write(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_appflowy::create_page::run(
                        &c,
                        require_str(&args, "title")?,
                        opt_str(&args, "markdown").unwrap_or(""),
                        opt_str(&args, "parent_view_id"),
                    )
                    .await,
                )
            }
        },
    );
    let c = client.clone();
    b.add_fn(ira_tools_appflowy::get_page::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(ira_tools_appflowy::get_page::run(&c, require_str(&args, "view_id")?).await)
        }
    });
    let c = client.clone();
    b.add_fn(
        ira_tools_appflowy::update_page::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_appflowy::update_page::run(
                        &c,
                        require_str(&args, "view_id")?,
                        opt_str(&args, "title"),
                        opt_str(&args, "markdown"),
                    )
                    .await,
                )
            }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_appflowy::delete_page::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_appflowy::delete_page::run(&c, require_str(&args, "view_id")?).await,
                )
            }
        },
    );
    let c = client;
    b.add_fn(
        ira_tools_appflowy::search_pages::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_appflowy::search_pages::run(
                        &c,
                        require_str(&args, "query")?,
                        opt_u64(&args, "limit"),
                    )
                    .await,
                )
            }
        },
    );
}

fn github(b: &mut Builder) {
    let Some(client) = ira_tools_github::Client::from_env() else {
        return;
    };
    tracing::info!("tools github listas");
    let c = client.clone();
    b.add_fn(ira_tools_github::create_issue::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_github::create_issue::run(
                    &c,
                    require_str(&args, "owner")?,
                    opt_str(&args, "repo").unwrap_or(""),
                    require_str(&args, "title")?,
                    opt_str(&args, "body"),
                    &opt_str_list(&args, "labels"),
                )
                .await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(ira_tools_github::get_issue::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            let number =
                opt_u64(&args, "number").or_else(|| opt_i64(&args, "number").map(|n| n as u64));
            let number = number.ok_or(crate::error::ToolError::MissingArg("number"))?;
            stringify(
                ira_tools_github::get_issue::run(
                    &c,
                    require_str(&args, "owner")?,
                    opt_str(&args, "repo").unwrap_or(""),
                    number,
                )
                .await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(ira_tools_github::list_issues::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_github::list_issues::run(
                    &c,
                    require_str(&args, "owner")?,
                    opt_str(&args, "repo").unwrap_or(""),
                    opt_str(&args, "state"),
                    opt_u64(&args, "limit"),
                )
                .await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(
        ira_tools_github::create_pull_request::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_github::create_pull_request::run(
                        &c,
                        require_str(&args, "owner")?,
                        opt_str(&args, "repo").unwrap_or(""),
                        require_str(&args, "title")?,
                        require_str(&args, "head")?,
                        opt_str(&args, "base"),
                        opt_str(&args, "body"),
                    )
                    .await,
                )
            }
        },
    );
    let c = client;
    b.add_fn(
        ira_tools_github::list_pull_requests::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_github::list_pull_requests::run(
                        &c,
                        require_str(&args, "owner")?,
                        opt_str(&args, "repo").unwrap_or(""),
                        opt_str(&args, "state"),
                        opt_u64(&args, "limit"),
                    )
                    .await,
                )
            }
        },
    );
}

fn google(b: &mut Builder) {
    let Some(client) = ira_tools_google::Client::from_env() else {
        return;
    };
    tracing::info!("tools google listas");
    let c = client.clone();
    b.add_fn(
        ira_tools_google::list_calendars::spec(),
        move |_ctx, _args| {
            let c = c.clone();
            async move { stringify(ira_tools_google::list_calendars::run(&c).await) }
        },
    );
    let c = client.clone();
    b.add_fn(ira_tools_google::list_events::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_google::list_events::run(
                    &c,
                    opt_str(&args, "calendar_id"),
                    opt_str(&args, "time_min"),
                    opt_str(&args, "time_max"),
                    opt_str(&args, "query"),
                    opt_u64(&args, "limit"),
                )
                .await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(ira_tools_google::create_event::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_google::create_event::run(
                    &c,
                    opt_str(&args, "calendar_id"),
                    require_str(&args, "summary")?,
                    opt_str(&args, "description"),
                    opt_str(&args, "start"),
                    opt_str(&args, "end"),
                    opt_str(&args, "date"),
                    opt_str(&args, "location"),
                )
                .await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(ira_tools_google::get_event::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_google::get_event::run(
                    &c,
                    opt_str(&args, "calendar_id"),
                    require_str(&args, "event_id")?,
                )
                .await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(ira_tools_google::update_event::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_google::update_event::run(
                    &c,
                    opt_str(&args, "calendar_id"),
                    require_str(&args, "event_id")?,
                    opt_str(&args, "summary"),
                    opt_str(&args, "description"),
                    opt_str(&args, "start"),
                    opt_str(&args, "end"),
                    opt_str(&args, "date"),
                    opt_str(&args, "location"),
                )
                .await,
            )
        }
    });
    let c = client;
    b.add_fn(ira_tools_google::delete_event::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            stringify(
                ira_tools_google::delete_event::run(
                    &c,
                    opt_str(&args, "calendar_id"),
                    require_str(&args, "event_id")?,
                )
                .await,
            )
        }
    });
}

fn home_assistant(b: &mut Builder) {
    let Some(client) = ira_tools_home_assistant::Client::from_env() else {
        return;
    };
    tracing::info!("tools home assistant listas");
    let c = client.clone();
    b.add_fn(
        ira_tools_home_assistant::list_states::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_home_assistant::list_states::run(
                        &c,
                        opt_str(&args, "domain"),
                        opt_u64(&args, "limit"),
                    )
                    .await,
                )
            }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_home_assistant::get_state::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_home_assistant::get_state::run(&c, require_str(&args, "entity_id")?)
                        .await,
                )
            }
        },
    );
    let c = client;
    b.add_fn(
        ira_tools_home_assistant::call_service::spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move {
                stringify(
                    ira_tools_home_assistant::call_service::run(
                        &c,
                        require_str(&args, "domain")?,
                        require_str(&args, "service")?,
                        opt_str(&args, "entity_id"),
                        args.get("data"),
                    )
                    .await,
                )
            }
        },
    );
}

fn callmebot(b: &mut Builder) {
    let Some(client) = ira_tools_callmebot::Client::from_env() else {
        return;
    };
    tracing::info!("tool whatsapp lista");
    b.add_fn(
        ira_tools_callmebot::send_message::spec(),
        move |_ctx, args| {
            let c = client.clone();
            async move {
                stringify(
                    ira_tools_callmebot::send_message::run(&c, require_str(&args, "text")?).await,
                )
            }
        },
    );
}

fn db(b: &mut Builder) {
    let Some(client) = ira_tools_db::Client::from_env() else {
        return;
    };
    tracing::info!("tools db listas");
    let c = client.clone();
    b.add_fn(ira_tools_db::list_tools::spec(), move |_ctx, _args| {
        let c = c.clone();
        async move { stringify(ira_tools_db::list_tools::run(&c).await) }
    });
    let c = client.clone();
    b.add_fn(ira_tools_db::invoke::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            let arguments = match args.get("arguments") {
                Some(serde_json::Value::String(s)) => {
                    serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({}))
                }
                Some(value) => value.clone(),
                None => serde_json::json!({}),
            };
            stringify(ira_tools_db::invoke::run(&c, require_str(&args, "tool")?, arguments).await)
        }
    });
}

fn kanban(b: &mut Builder) {
    let Some(client) = ira_tools_kanban::Client::from_env() else {
        return;
    };
    tracing::info!("tools kanban listas");
    let c = client.clone();
    b.add_fn(ira_tools_kanban::list_tools::spec(), move |_ctx, _args| {
        let c = c.clone();
        async move { stringify(ira_tools_kanban::list_tools::run(&c).await) }
    });
    let c = client.clone();
    b.add_fn(ira_tools_kanban::invoke::spec(), move |_ctx, args| {
        let c = c.clone();
        async move {
            let arguments = match args.get("arguments") {
                Some(serde_json::Value::String(s)) => {
                    serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({}))
                }
                Some(value) => value.clone(),
                None => serde_json::json!({}),
            };
            stringify(
                ira_tools_kanban::invoke::run(&c, require_str(&args, "tool")?, arguments).await,
            )
        }
    });
    let c = client.clone();
    b.add_fn(
        ira_tools_kanban::tasks::list_tasks_spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move { stringify(ira_tools_kanban::tasks::list_tasks(&c, args).await) }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_kanban::tasks::get_task_spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move { stringify(ira_tools_kanban::tasks::get_task(&c, args).await) }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_kanban::tasks::create_task_spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move { stringify(ira_tools_kanban::tasks::create_task(&c, args).await) }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_kanban::tasks::update_task_spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move { stringify(ira_tools_kanban::tasks::update_task(&c, args).await) }
        },
    );
    let c = client.clone();
    b.add_fn(
        ira_tools_kanban::tasks::archive_task_spec(),
        move |_ctx, args| {
            let c = c.clone();
            async move { stringify(ira_tools_kanban::tasks::archive_task(&c, args).await) }
        },
    );
    let c = client;
    b.add_fn(
        ira_tools_kanban::tasks::summary_spec(),
        move |_ctx, _args| {
            let c = c.clone();
            async move { stringify(ira_tools_kanban::tasks::summary(&c).await) }
        },
    );
}
