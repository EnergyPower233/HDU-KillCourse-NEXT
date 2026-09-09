use crate::{
    client::{self, Outcome, SchoolClient},
    model::{self, Action, Course, Mode, Progress, Settings, TaskList},
    state::{publish, AppState},
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub(crate) async fn wait_or_stop(token: &CancellationToken, millis: u64) -> bool {
    tokio::select! { biased; _ = token.cancelled() => false, _ = tokio::time::sleep(Duration::from_millis(millis)) => true }
}

pub(crate) fn method_label(method: model::LoginMethod) -> &'static str {
    match method {
        model::LoginMethod::Cas => "统一身份认证",
        model::LoginMethod::Newjw => "教务账号",
        model::LoginMethod::Qrcode => "钉钉扫码",
        model::LoginMethod::Cookie => "已有 Cookie",
    }
}

pub(crate) async fn run_tasks(
    state: &AppState,
    mut client: SchoolClient,
    stored: &model::StoredCredentials,
    ua: &model::UaConfig,
    list: &TaskList,
    settings: &Settings,
    token: &CancellationToken,
) -> Result<(), String> {
    publish(
        state,
        Progress::new("", "running", &format!("任务启动：清单「{}」", list.name)),
    );
    let delay = settings.delay_ms()?;
    if delay > 0 {
        for t in &list.tasks {
            if let Some(course) = &t.course {
                publish(
                    state,
                    Progress::for_course(
                        course,
                        Action::Select,
                        "waiting",
                        "等待计划时间（UTC+8）",
                    ),
                );
            }
            for drop in &t.drops {
                publish(
                    state,
                    Progress::for_course(drop, Action::Cancel, "waiting", "等待计划时间（UTC+8）"),
                );
            }
        }
        // Optional freshness re-login: N seconds before the scheduled start,
        // run the stored login methods in the user's preferred order so the
        // session is fresh when the task fires.
        let pre = if settings.relogin_before_secs > 0 {
            (settings.relogin_before_secs * 1000).min(delay)
        } else {
            0
        };
        if pre > 0 {
            let first = delay - pre;
            if first > 0 && !wait_or_stop(token, first).await {
                return Ok(());
            }
            publish(
                state,
                Progress::new(
                    "",
                    "running",
                    &format!(
                        "距离开始还有 {} 秒，按你的顺序重新登录教务系统",
                        settings.relogin_before_secs
                    ),
                ),
            );
            let attempt = tokio::select! {
                biased;
                _ = token.cancelled() => None,
                r = client::SchoolClient::login_with_order(stored, settings, ua) => Some(r),
            };
            match attempt {
                Some(Ok((fresh, method))) => {
                    client = fresh;
                    publish(
                        state,
                        Progress::new(
                            "",
                            "success",
                            &format!(
                                "提前重新登录成功（{}），任务将使用新会话",
                                method_label(method)
                            ),
                        ),
                    );
                }
                Some(Err(e)) => publish(
                    state,
                    Progress::new(
                        "",
                        "error",
                        &format!("提前重新登录失败：{e}，将沿用现有会话"),
                    ),
                ),
                None => return Ok(()),
            }
            if !wait_or_stop(token, pre).await {
                return Ok(());
            }
        } else if !wait_or_stop(token, delay).await {
            return Ok(());
        }
    }
    let config = tokio::select! { biased; _ = token.cancelled() => return Ok(()), result = client.body_config() => result? };
    let mut pending = list.tasks.clone();
    loop {
        if token.is_cancelled() {
            return Ok(());
        }
        if matches!(list.mode, Mode::Watch) {
            let mut ready: Vec<Course> = Vec::new();
            // Bounded parallel reads. Mutations below are deliberately serialized.
            for batch in pending.chunks(4) {
                let mut queries = tokio::task::JoinSet::new();
                for task in batch {
                    // Watch mode is validated to contain pure select tasks only.
                    let course = task
                        .course
                        .as_ref()
                        .expect("watch tasks always select a course");
                    publish(
                        state,
                        Progress::for_course(course, Action::Select, "querying", "正在查询余量"),
                    );
                    let (c, s, t) = (client.clone(), settings.clone(), course.clone());
                    queries.spawn(async move {
                        let result = c.available(&s, &t).await;
                        (t, result)
                    });
                }
                while !queries.is_empty() {
                    let row = tokio::select! { biased; _ = token.cancelled() => return Ok(()), row = queries.join_next() => row };
                    let (course, result) = row.unwrap().map_err(|_| "查询任务异常结束")?;
                    match result {
                        Ok(true) => ready.push(course),
                        Ok(false) => publish(
                            state,
                            Progress::for_course(
                                &course,
                                Action::Select,
                                "waiting",
                                "暂无余量，等待下一次查询",
                            ),
                        ),
                        Err(e) => {
                            publish(
                                state,
                                Progress::for_course(&course, Action::Select, "error", &e),
                            );
                            return Err("查询失败，已停止本轮；请检查网络或重新登录".into());
                        }
                    }
                }
            }
            for course in ready {
                if token.is_cancelled() {
                    return Ok(());
                }
                submit_select(state, &client, settings, &config, &course, token).await?;
                pending.retain(|t| {
                    t.course
                        .as_ref()
                        .map(|c| c.jxbmc != course.jxbmc)
                        .unwrap_or(true)
                });
            }
        } else {
            // Once mode: process tasks strictly in list order, dropping first
            // and selecting afterwards. A consumed task is removed as we go.
            while !pending.is_empty() {
                let task = pending.remove(0);
                if token.is_cancelled() {
                    return Ok(());
                }
                run_once_task(state, &client, settings, &config, &task, token).await?;
            }
        }
        if pending.is_empty() || matches!(list.mode, Mode::Once) {
            return Ok(());
        }
        let wait = client::jittered_wait(
            settings.interval_ms,
            settings.jitter_ms,
            settings.jitter_seed,
        );
        if !wait_or_stop(token, wait).await {
            return Ok(());
        }
    }
}

/// Submit one select request. Err means "stop the whole round" (result
/// unknown); a rejected select is logged and the task is consumed.
async fn submit_select(
    state: &AppState,
    client: &SchoolClient,
    settings: &Settings,
    config: &std::collections::HashMap<String, String>,
    course: &Course,
    token: &CancellationToken,
) -> Result<(), String> {
    let body = tokio::select! { biased; _ = token.cancelled() => return Ok(()), result = client.prepare(settings, course, config) => result };
    let body = match body {
        Ok(body) => body,
        Err(e) => {
            publish(
                state,
                Progress::for_course(course, Action::Select, "error", &e),
            );
            return Ok(());
        }
    };
    if token.is_cancelled() {
        return Ok(());
    }
    publish(
        state,
        Progress::for_course(
            course,
            Action::Select,
            "submitting",
            "正在提交，请等待学校回复",
        ),
    );
    // Do not drop a mutation future on cancellation: retain its result, then stop.
    if token.is_cancelled() {
        return Ok(());
    }
    let outcome = client.submit(&Action::Select, &body).await;
    match outcome {
        Outcome::Success => {
            publish(
                state,
                Progress::for_course(
                    course,
                    Action::Select,
                    "success",
                    "学校已返回成功，请以教务系统已选记录为准",
                ),
            );
            Ok(())
        }
        Outcome::Rejected(message) => {
            publish(
                state,
                Progress::for_course(course, Action::Select, "rejected", &message),
            );
            Ok(())
        }
        Outcome::Unknown => {
            publish(
                state,
                Progress::for_course(
                    course,
                    Action::Select,
                    "unknown",
                    "提交结果不明，已暂停全部任务。请先在教务系统核实，勿直接重复提交",
                ),
            );
            Err("出现结果不明的提交，请核实真实课表后再操作".into())
        }
    }
}

/// Submit one drop request. Ok(true) = dropped, Ok(false) = failed (skip the
/// paired select), Err = stop the whole round (result unknown).
async fn submit_drop(
    state: &AppState,
    client: &SchoolClient,
    settings: &Settings,
    config: &std::collections::HashMap<String, String>,
    drop: &Course,
    token: &CancellationToken,
) -> Result<bool, String> {
    let body = tokio::select! { biased; _ = token.cancelled() => return Ok(false), result = client.prepare(settings, drop, config) => result };
    let body = match body {
        Ok(body) => body,
        Err(e) => {
            publish(
                state,
                Progress::for_course(drop, Action::Cancel, "error", &format!("退课准备失败：{e}")),
            );
            return Ok(false);
        }
    };
    if token.is_cancelled() {
        return Ok(false);
    }
    publish(
        state,
        Progress::for_course(
            drop,
            Action::Cancel,
            "submitting",
            "正在提交退课，请等待学校回复",
        ),
    );
    if token.is_cancelled() {
        return Ok(false);
    }
    let outcome = client.submit(&Action::Cancel, &body).await;
    match outcome {
        Outcome::Success => {
            publish(
                state,
                Progress::for_course(
                    drop,
                    Action::Cancel,
                    "success",
                    "退课：学校已返回成功，请以教务系统已选记录为准",
                ),
            );
            Ok(true)
        }
        Outcome::Rejected(message) => {
            publish(
                state,
                Progress::for_course(
                    drop,
                    Action::Cancel,
                    "rejected",
                    &format!("退课被拒绝：{message}"),
                ),
            );
            Ok(false)
        }
        Outcome::Unknown => {
            publish(
                state,
                Progress::for_course(
                    drop,
                    Action::Cancel,
                    "unknown",
                    "退课结果不明，已暂停全部任务。请先在教务系统核实",
                ),
            );
            Err("出现结果不明的退课，请核实真实课表后再操作".into())
        }
    }
}

/// One once-mode task: drop everything first, then select. Err = stop round.
async fn run_once_task(
    state: &AppState,
    client: &SchoolClient,
    settings: &Settings,
    config: &std::collections::HashMap<String, String>,
    task: &model::CourseTask,
    token: &CancellationToken,
) -> Result<(), String> {
    for drop in &task.drops {
        if token.is_cancelled() {
            return Ok(());
        }
        let dropped = submit_drop(state, client, settings, config, drop, token).await?;
        if !dropped {
            if let Some(course) = &task.course {
                publish(
                    state,
                    Progress::for_course(
                        course,
                        Action::Select,
                        "error",
                        "前置退课未全部成功，跳过本次选课",
                    ),
                );
            }
            return Ok(());
        }
    }
    if let Some(course) = &task.course {
        if token.is_cancelled() {
            return Ok(());
        }
        submit_select(state, client, settings, config, course, token).await?;
    }
    Ok(())
}
