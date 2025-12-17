use std::{cell::RefCell, sync::OnceLock};

use anyhow::{Context, Error, Result};
use async_lock::Semaphore;
use dioxus::prelude::*;
use futures::channel::oneshot::{channel, Sender};
use wasm_bindgen::prelude::*;
use web_sys::{
    js_sys::{self, Array, Uint8Array},
    MessageEvent, Worker, WorkerOptions, WorkerType,
};

#[derive(Debug, Clone)]
pub struct TranscodeResult {
    pub filename: String,
    pub data: Box<[u8]>,
}

const WORKER_DIR: Asset = asset!("/assets/worker");
const N_WORKERS: usize = 4;

thread_local! {
    static WORKERS: RefCell<Option<Vec<Worker>>> = const { RefCell::new(None) };
}
static FREE_WORKERS: OnceLock<Semaphore> = OnceLock::new();

pub(crate) async fn transcode(
    input: Box<[u8]>,
    progress: impl FnMut(usize, usize),
) -> Result<TranscodeResult> {
    let result: JsValue = transcode_in_worker(input, progress).await?;

    // Extract filename and data from the result object
    let filename = get_prop(&result, "filename")
        .and_then(|v| v.as_string())
        .and_then(|v| v.strip_suffix(".wav").map(ToString::to_string))
        .ok_or_else(|| anyhow::anyhow!("Missing filename in transcode result"))?;

    let data_value = get_prop(&result, "data")
        .ok_or_else(|| anyhow::anyhow!("Missing data in transcode result"))?;

    let u8_array = Uint8Array::new(&data_value);
    let mut vec = vec![0u8; u8_array.length() as usize];
    u8_array.copy_to(vec.as_mut_slice());

    Ok(TranscodeResult {
        filename,
        data: vec.into(),
    })
}

async fn transcode_in_worker(
    input: Box<[u8]>,
    mut progress: impl FnMut(usize, usize),
) -> Result<JsValue> {
    with_worker(|worker: Worker| async move {
        let u8_array = Uint8Array::new_from_slice(&input);
        let buffer = u8_array.buffer();

        let (tx, rx) = channel::<Result<JsValue>>();
        let mut tx = Some(tx);
        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            process_message_from_worker(event.data(), &mut progress, &mut tx);
        }) as Box<dyn FnMut(MessageEvent)>);
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        worker
            .post_message_with_transfer(&buffer, &Array::of1(&buffer))
            .map_err(to_error)
            .context("transmitting payload to worker")?;

        let result = rx
            .await
            .context("receive error")?
            .context("worker communication error")?;
        worker.set_onmessage(None);

        Ok(result) as Result<JsValue>
    })
    .await?
}

fn process_message_from_worker(
    data: JsValue,
    progress: &mut impl FnMut(usize, usize),
    tx: &mut Option<Sender<Result<JsValue>>>,
) {
    if let Some(progress_data) = get_prop(&data, "progress") {
        let current: Option<f64> = try { get_prop(&progress_data, "current")?.as_f64()? };
        let total: Option<f64> = try { get_prop(&progress_data, "total")?.as_f64()? };

        if let (Some(current), Some(total)) = (current, total) {
            progress(current as usize, total as usize);
        }
    } else if let Some(result) = get_prop(&data, "result") {
        if let Some(tx) = tx.take() {
            if tx.send(Ok(result)).is_err() {
                web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(
                    "Failed to send transcode result - receiver dropped",
                ));
            }
        }
    } else if let Some(error) = get_prop(&data, "error") {
        if let Some(tx) = tx.take() {
            if tx.send(Err(to_error(error))).is_err() {
                web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(
                    "Failed to send transcode error - receiver dropped",
                ));
            }
        }
    }
}

fn new_worker() -> Result<Worker> {
    let worker_options = WorkerOptions::new();
    worker_options.set_type(WorkerType::Module);

    let worker = Worker::new_with_options(&format!("{}/worker.js", WORKER_DIR), &worker_options)
        .map_err(to_error)?;

    Ok(worker)
}

fn init_workers() -> Result<()> {
    let mut result = Ok(());

    WORKERS.with_borrow_mut(|option| {
        if option.is_some() {
            return;
        }

        let mut vec = Vec::with_capacity(N_WORKERS);
        for _ in 0..N_WORKERS {
            match new_worker() {
                Ok(worker) => vec.push(worker),
                Err(err) => {
                    result = Err(err);
                    return;
                }
            }
        }
        *option = Some(vec);
    });

    result
}

async fn with_worker<F, R>(f: F) -> Result<R>
where
    F: AsyncFnOnce(Worker) -> R,
{
    init_workers()?;

    let sem = FREE_WORKERS.get_or_init(|| Semaphore::new(N_WORKERS));
    let _permit = sem.acquire().await;

    let worker = WORKERS.with_borrow_mut(|workers| {
        workers
            .as_mut()
            .and_then(|w| w.pop())
            .ok_or_else(|| anyhow::anyhow!("No available workers"))
    })?;
    let res = f(worker.clone()).await;

    WORKERS.with_borrow_mut(move |workers| {
        if let Some(workers) = workers.as_mut() {
            workers.push(worker);
        }
    });

    Ok(res)
}

fn to_error(value: JsValue) -> Error {
    Error::msg(
        js_sys::Object::to_string(&value.into())
            .as_string()
            .unwrap_or_else(|| "[error]".to_string()),
    )
}

fn get_prop(obj: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .and_then(|value| {
            if value.is_undefined() {
                None
            } else {
                Some(value)
            }
        })
}
