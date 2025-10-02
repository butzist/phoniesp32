use std::{cell::RefCell, sync::OnceLock};

use dioxus::prelude::*;
use tokio::sync::{oneshot::channel, Semaphore};
use wasm_bindgen::prelude::*;
use web_sys::{
    js_sys::{self, Array, Uint8Array},
    MessageEvent, Worker, WorkerOptions, WorkerType,
};

const WORKER_DIR: Asset = asset!("/assets/worker");
const N_WORKERS: usize = 4;

thread_local! {
    static WORKERS: RefCell<Option<Vec<Worker>>> = const { RefCell::new(None) };
}
static FREE_WORKERS: OnceLock<Semaphore> = OnceLock::new();

pub(crate) async fn transcode(
    input: Box<[u8]>,
    mut progress: impl FnMut(usize, usize),
) -> Result<Box<[u8]>, String> {
    let result: JsValue = with_worker(|worker: Worker| async move {
        let u8_array = Uint8Array::new_from_slice(&input);
        let buffer = u8_array.buffer();

        let (tx, rx) = channel();
        let mut tx = Some(tx);
        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            // TODO error handling
            let data = event.data();
            if let Some(progress_data) = get_prop(&data, "progress") {
                let current = get_prop(&progress_data, "current")
                    .unwrap()
                    .as_f64()
                    .unwrap();
                let total = get_prop(&progress_data, "total").unwrap().as_f64().unwrap();
                progress(current as usize, total as usize);
            } else if let Some(result) = get_prop(&data, "result") {
                if let Some(tx) = tx.take() {
                    tx.send(result).unwrap();
                }
            } else if let Some(error) = get_prop(&data, "error") {
                panic!("{}", to_string(error));
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        worker
            .post_message_with_transfer(&buffer, &Array::of1(&buffer))
            .unwrap();

        let result = rx.await.unwrap();
        worker.set_onmessage(None);
        Ok(result)
    })
    .await?
    .map_err(to_string)?;

    let u8_array = Uint8Array::new(&result);
    let mut vec = vec![0u8; u8_array.length() as usize];
    u8_array.copy_to(vec.as_mut_slice());

    Ok(vec.into())
}

fn new_worker() -> Result<Worker, String> {
    let worker_options = WorkerOptions::new();
    worker_options.set_type(WorkerType::Module);

    let worker = Worker::new_with_options(&format!("{}/worker.js", WORKER_DIR), &worker_options)
        .map_err(to_string)?;

    Ok(worker)
}

fn init_workers() -> Result<(), String> {
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

async fn with_worker<F, R>(f: F) -> Result<R, String>
where
    F: AsyncFnOnce(Worker) -> R,
{
    init_workers()?;

    let sem = FREE_WORKERS.get_or_init(|| Semaphore::new(N_WORKERS));
    let _permit = sem.acquire().await.unwrap();

    let worker = WORKERS.with_borrow_mut(|workers| workers.as_mut().unwrap().pop().unwrap());
    let res = f(worker.clone()).await;

    WORKERS.with_borrow_mut(move |workers| workers.as_mut().unwrap().push(worker));

    Ok(res)
}

fn to_string(value: JsValue) -> String {
    js_sys::Object::to_string(&value.into())
        .as_string()
        .unwrap_or_else(|| "[error]".to_string())
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
