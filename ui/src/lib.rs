#![recursion_limit = "128"]
extern crate strum;
#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate yew;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate stdweb;

use failure::Error;
use std::time::Duration;
use strum::IntoEnumIterator;
use yew::format::{Json, Nothing};
use yew::prelude::*;
use yew::services::fetch::{FetchService, FetchTask, Request, Response};
use yew::services::interval::IntervalService;
use yew::services::storage::{Area, StorageService};
use yew::services::Task;

const KEY: &'static str = "yew.todomvc.self";

pub struct Model {
    fetch: FetchService,
    storage: StorageService,
    state: State,
    _ticker: Box<Task>,
    gotten: Option<FetchTask>,
}

#[derive(Serialize, Deserialize)]
pub struct State {
    entries: Vec<Entry>,
    filter: Filter,
    value: String,
    edit_value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    description: String,
    completed: bool,
    editing: bool,
}

pub enum Msg {
    Add,
    ClearCompleted,
    Edit(usize),
    InitialSync(Vec<Entry>),
    Update(String),
    UpdateEdit(String),
    Remove(usize),
    SetFilter(Filter),
    ToggleAll,
    ToggleEdit(usize),
    Toggle(usize),
    Nope,
    Tick,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    // set up and Box all the required callbacks, initialize services (some of
    // which we can pass cloned callbacks to in the event loop)
    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut storage = StorageService::new(Area::Local);
        let mut interval = IntervalService::new();
        let cb = link.send_back(|_| Msg::Tick);
        let handle = interval.spawn(Duration::from_secs(5), cb);
        let mut fetch = FetchService::new();
        let entries = {
            if let Json(Ok(restored_model)) = storage.restore(KEY) {
                restored_model
            } else {
                Vec::new()
            }
        };

        // The full signature of the send_back method on ComponentLink
        //
        //  impl<COMP> ComponentLink<COMP>
        //  where
        //      COMP: Component + Renderable<COMP>,
        //
        //       ... 
        //
        //       pub fn send_back<F, IN>(&self, function: F) -> Callback<IN>
        //       where
        //           F: Fn(IN) -> COMP::Message + 'static,
        //
        //       ...
        //
        // We need a separate callback for each of the different Msg types that
        // could be yielded from initial_sync_cb
        //
        // TODO: This is corny. Make this nicer.
        let emitter = link.send_back(Msg::InitialSync);
        let emitter2 = link.send_back(|_| Msg::Nope);

        let url = format!("http://[::]:8000/tasks");
        let request = Request::get(url.as_str()).body(Nothing).unwrap();
        let initial_sync_cb = link.send_back(move |resp: Response<Json<Result<Vec<Entry>, Error>>>| {
            let (meta, Json(data)) = resp.into_parts();
            if meta.status.is_success() {
                console!(log, format!("got data from server: {:?}", data));
                emitter.emit(data.expect("error response from GET /tasks"))
            } else {
                console!(log, "initial sync failed");
                emitter2.emit(Msg::Nope)
            }
            Msg::Nope
        }); 
        let task = Some(fetch.fetch(request, initial_sync_cb));
        let state = State {
            entries,
            filter: Filter::All,
            value: "".into(),
            edit_value: "".into(),
        };
        Model {
            fetch,
            storage,
            state,
            _ticker: Box::new(handle),
            // we have to move this into our struct to prevent it from being
            // dropped
            gotten: task,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Add => {
                let entry = Entry {
                    description: self.state.value.clone(),
                    completed: false,
                    editing: false,
                };
                self.state.entries.push(entry);
                self.state.value = "".to_string();
            }
            Msg::Edit(idx) => {
                let edit_value = self.state.edit_value.clone();
                self.state.complete_edit(idx, edit_value);
                self.state.edit_value = "".to_string();
            }
            Msg::InitialSync(data) => {
                console!(log, "got data from server: ");
                self.state.entries = data;
            },
            Msg::Update(val) => {
                println!("Input: {}", val);
                self.state.value = val;
            }
            Msg::UpdateEdit(val) => {
                println!("Input: {}", val);
                self.state.edit_value = val;
            }
            Msg::Remove(idx) => {
                self.state.remove(idx);
            }
            Msg::SetFilter(filter) => {
                self.state.filter = filter;
            }
            Msg::Tick => {
                console!(log, "tock");
                // make a callback (or box it and put it on our Model)
                let url = format!("http://[::]:8000/tasks");
                let handler = move |response: Response<Json<Result<(), Error>>>| {
                    let (meta, _) = response.into_parts();
                    if !meta.status.is_success() {
                        // format_err! is a macro in crate `failure`
                        // callback.emit(Err(format_err!(
                        //     "{}: error getting profile https://gravatar.com/",
                        //     meta.status
                        // )))
                    }
                };
                let entries = self.state.entries.clone();
                let as_json = json!{ entries };
                let request = Request::post(url.as_str())
                    .header("Content-Type", "application/json")
                    .body(Ok(as_json.to_string()))
                    .unwrap();
                self.fetch.fetch(request, handler.into());
            }
            Msg::ToggleEdit(idx) => {
                self.state.edit_value = self.state.entries[idx].description.clone();
                self.state.toggle_edit(idx);
            }
            Msg::ToggleAll => {
                let status = !self.state.is_all_completed();
                self.state.toggle_all(status);
            }
            Msg::Toggle(idx) => {
                self.state.toggle(idx);
            }
            Msg::ClearCompleted => {
                self.state.clear_completed();
            }
            Msg::Nope => {}
        }
        self.storage.store(KEY, Json(&self.state.entries));
        true
    }
}

impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
        html! {
            <div class="todomvc-wrapper",>
                <section class="todoapp",>
                    <header class="header",>
                        <h1>{ "todos" }</h1>
                        { self.view_input() }
                    </header>
                    <section class="main",>
                        <input class="toggle-all", type="checkbox", checked=self.state.is_all_completed(), onclick=|_| Msg::ToggleAll, />
                        <ul class="todo-list",>
                            { for self.state.entries.iter().filter(|e| self.state.filter.fit(e)).enumerate().map(view_entry) }
                        </ul>
                    </section>
                    <footer class="footer",>
                        <span class="todo-count",>
                            <strong>{ self.state.total() }</strong>
                            { " item(s) left" }
                        </span>
                        <ul class="filters",>
                            { for Filter::iter().map(|flt| self.view_filter(flt)) }
                        </ul>
                        <button class="clear-completed", onclick=|_| Msg::ClearCompleted,>
                            { format!("Clear completed ({})", self.state.total_completed()) }
                        </button>
                    </footer>
                </section>
                <footer class="info",>
                    <p>{ "Double-click to edit a todo" }</p>
                    <p>{ "Written by " }<a href="https://github.com/DenisKolodin/", target="_blank",>{ "Denis Kolodin" }</a></p>
                    <p>{ "Part of " }<a href="http://todomvc.com/", target="_blank",>{ "TodoMVC" }</a></p>
                </footer>
            </div>
        }
    }
}

impl Model {
    fn view_filter(&self, filter: Filter) -> Html<Model> {
        let flt = filter.clone();
        html! {
            <li>
                <a class=if self.state.filter == flt { "selected" } else { "not-selected" },
                   href=&flt,
                   onclick=|_| Msg::SetFilter(flt.clone()),>
                    { filter }
                </a>
            </li>
        }
    }

    fn view_input(&self) -> Html<Model> {
        html! {
            // You can use standard Rust comments. One line:
            // <li></li>
            <input class="new-todo",
                   placeholder="What needs to be done?",
                   value=&self.state.value,
                   oninput=|e| Msg::Update(e.value),
                   onkeypress=|e| {
                       if e.key() == "Enter" { Msg::Add } else { Msg::Nope }
                   }, />
            /* Or multiline:
            <ul>
                <li></li>
            </ul>
            */
        }
    }
}

fn view_entry((idx, entry): (usize, &Entry)) -> Html<Model> {
    html! {
        <li class=if entry.editing == true { "editing" } else { "" },>
            <div class="view",>
                <input class="toggle", type="checkbox", checked=entry.completed, onclick=|_| Msg::Toggle(idx), />
                <label ondoubleclick=|_| Msg::ToggleEdit(idx),>{ &entry.description }</label>
                <button class="destroy", onclick=|_| Msg::Remove(idx), />
            </div>
            { view_entry_edit_input((idx, &entry)) }
        </li>
    }
}

fn view_entry_edit_input((idx, entry): (usize, &Entry)) -> Html<Model> {
    if entry.editing == true {
        html! {
            <input class="edit",
                   type="text",
                   value=&entry.description,
                   oninput=|e| Msg::UpdateEdit(e.value),
                   onblur=|_| Msg::Edit(idx),
                   onkeypress=|e| {
                      if e.key() == "Enter" { Msg::Edit(idx) } else { Msg::Nope }
                   }, />
        }
    } else {
        html! { <input type="hidden", /> }
    }
}

#[derive(EnumIter, ToString, Clone, PartialEq, Serialize, Deserialize)]
pub enum Filter {
    All,
    Active,
    Completed,
}

impl<'a> Into<Href> for &'a Filter {
    fn into(self) -> Href {
        match *self {
            Filter::All => "#/".into(),
            Filter::Active => "#/active".into(),
            Filter::Completed => "#/completed".into(),
        }
    }
}

impl Filter {
    fn fit(&self, entry: &Entry) -> bool {
        match *self {
            Filter::All => true,
            Filter::Active => !entry.completed,
            Filter::Completed => entry.completed,
        }
    }
}

impl State {
    fn total(&self) -> usize {
        self.entries.len()
    }

    fn total_completed(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| Filter::Completed.fit(e))
            .count()
    }

    fn is_all_completed(&self) -> bool {
        let mut filtered_iter = self
            .entries
            .iter()
            .filter(|e| self.filter.fit(e))
            .peekable();

        if filtered_iter.peek().is_none() {
            return false;
        }

        filtered_iter.all(|e| e.completed)
    }

    fn toggle_all(&mut self, value: bool) {
        for entry in self.entries.iter_mut() {
            if self.filter.fit(entry) {
                entry.completed = value;
            }
        }
    }

    fn clear_completed(&mut self) {
        let entries = self
            .entries
            .drain(..)
            .filter(|e| Filter::Active.fit(e))
            .collect();
        self.entries = entries;
    }

    fn toggle(&mut self, idx: usize) {
        let filter = self.filter.clone();
        let mut entries = self
            .entries
            .iter_mut()
            .filter(|e| filter.fit(e))
            .collect::<Vec<_>>();
        let entry = entries.get_mut(idx).unwrap();
        entry.completed = !entry.completed;
    }

    fn toggle_edit(&mut self, idx: usize) {
        let filter = self.filter.clone();
        let mut entries = self
            .entries
            .iter_mut()
            .filter(|e| filter.fit(e))
            .collect::<Vec<_>>();
        let entry = entries.get_mut(idx).unwrap();
        entry.editing = !entry.editing;
    }

    fn complete_edit(&mut self, idx: usize, val: String) {
        let filter = self.filter.clone();
        let mut entries = self
            .entries
            .iter_mut()
            .filter(|e| filter.fit(e))
            .collect::<Vec<_>>();
        let entry = entries.get_mut(idx).unwrap();
        entry.description = val;
        entry.editing = !entry.editing;
    }

    fn remove(&mut self, idx: usize) {
        let idx = {
            let filter = self.filter.clone();
            let entries = self
                .entries
                .iter()
                .enumerate()
                .filter(|&(_, e)| filter.fit(e))
                .collect::<Vec<_>>();
            let &(idx, _) = entries.get(idx).unwrap();
            idx
        };
        self.entries.remove(idx);
    }
}
