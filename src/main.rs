


use anyhow::Error;

use serde_derive::{Deserialize, Serialize};

use wasm_bindgen::JsCast;

use wasm_bindgen::prelude::*;

use web_sys::{ ErrorEvent, MessageEvent, WebSocket, HtmlCanvasElement, WebGlRenderingContext as GL};


use state::{Entry, Filter, State};
use strum::IntoEnumIterator;
use yew::format::{Json, Nothing};
use yew::web_sys::HtmlInputElement as InputElement;
use yew::{html, Component, ComponentLink, Html, InputData, NodeRef, ShouldRender};
use yew::{events::KeyboardEvent, Classes};
use yew_services::storage::{Area, StorageService};
use yew_services::render::RenderTask;
use yew_services::RenderService;
use yew::prelude::*;
use yew_services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use yew_services::fetch::{FetchService, FetchTask, Request, Response};


macro_rules! c {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}



#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}


mod state;


const KEY: &str = "yew.keystone.self";


// Much material copied from the example: dashboard



type AsBinary = bool;

pub enum Format {
    Json,
}

pub enum WsAction {
    Connect,
    SendData(AsBinary),
    Disconnect,
    Lost,
}

impl From<WsAction> for Msg {
    fn from(action: WsAction) -> Self {
        Msg::WsAction(action)
    }
}



// This type is used to parse data from `./static/data.json` file and 
// have to correspond the data layout from that file.
#[derive(Deserialize, Debug)]
pub struct DataFromFile {
    value: u32,
}


// This type is used as a request which sent to websocket connection.
#[derive(Serialize, Debug)]
struct WsRequest {
    value: u32,
}


// This type is an expected response from a websocket connection.
#[derive(Deserialize, Debug)]
pub struct WsResponse {
    value: u32,
}







pub enum Msg {
    FetchData(Format, AsBinary),
    WsAction(WsAction),
    FetchReady(Result<DataFromFile, Error>),
    WsReady(Result<WsResponse, Error>),
    Render(f64),
    Add,
    Edit(usize),
    Update(String),
    UpdateEdit(String),
    Remove(usize),
    SetFilter(Filter),
    ToggleAll,
    ToggleEdit(usize),
    Toggle(usize),
    ClearCompleted,
    Focus,
}

pub struct Model {
    link: ComponentLink<Self>,
    storage: StorageService,
    state: State,
    focus_ref: NodeRef,
    canvas: Option<HtmlCanvasElement>,
    gl: Option<GL>,
    node_ref: NodeRef,
    render_loop: Option<RenderTask>,

    data: Option<u32>,
    _ft: Option<FetchTask>,
    ws: Option<WebSocketTask>,
}




// #[wasm_bindgen(start)]
pub fn start_websocket() -> Result<(), JsValue> {
    let ws = WebSocket::new("ws://pendragon.is/")?;

    let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
        let response = e
            .data()
            .as_string()
            .expect("Can't convert received data to a string");
        c!("message event, received data: {:?}", response);
    }) as Box<dyn FnMut(MessageEvent)>);

    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));

    onmessage_callback.forget();

    let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
        c!("error event: {:?}", e);
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();
    let cloned_ws = ws.clone();
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        c!("socket opened");
        match cloned_ws.send_with_str("ping") {
            Ok(_) => c!("message successfully sent"),
            Err(err) => c!("error sending message: {:?}", err),
        }
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();
    Ok(())
}










impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let storage = StorageService::new(Area::Local).expect("storage was disabled by the user");
        let entries = {
            if let Json(Ok(restored_model)) = storage.restore(KEY) {
                restored_model
            } else {
                Vec::new()
            }
        };
        let state = State {
            entries,
            filter: Filter::All,
            value: "".into(),
            edit_value: "".into(),
        };
        let focus_ref = NodeRef::default();
        Self {
            link,
            storage,
            state,
            focus_ref,
            canvas: None,
            gl: None,
            node_ref: NodeRef::default(),
            render_loop: None,
            
            data: None,
            _ft: None,
            ws: None,

        }
    }

    fn rendered(&mut self, first_render: bool) {
        let canvas = self.node_ref.cast::<HtmlCanvasElement>().unwrap();

        let gl: GL = canvas
            .get_context("webgl")
            .unwrap()
            .unwrap()
            .dyn_into()
            .unwrap();

        self.canvas = Some(canvas);
        self.gl = Some(gl);

        if first_render {
            let render_frame = self.link.callback(Msg::Render);
            let handle = RenderService::request_animation_frame(render_frame);

            self.render_loop = Some(handle);
        }
    }


    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {

            Msg::FetchData(format, binary) => {
                let task = match format {
                    Format::Json => self.fetch_json(binary),
    
                };
                self._ft = Some(task);
                true
            }

            Msg::WsAction(action) => match action {
                WsAction::Connect => {
                    let callback = self.link.callback(|Json(data)| Msg::WsReady(data));
                    let notification = self.link.batch_callback(|status| match status {
                        WebSocketStatus::Opened => None,
                        WebSocketStatus::Closed | WebSocketStatus::Error => {
                            Some(WsAction::Lost.into())
                        }
                    });
                    let task =
                        WebSocketService::connect("ws://pendragon.is", callback, notification)
                            .unwrap();
                    self.ws = Some(task);
                    true
                }
                WsAction::SendData(binary) => {
                    let request = WsRequest { value: 321 };
                    if binary {
                        self.ws.as_mut().unwrap().send_binary(Json(&request));
                    } else {
                        self.ws.as_mut().unwrap().send(Json(&request));
                    }
                    false
                }
                WsAction::Disconnect => {
                    self.ws.take();
                    true
                }
                WsAction::Lost => {
                    self.ws = None;
                    true
                }
            }

            Msg::FetchReady(response) => {
                self.data = response.map(|data| data.value).ok();
                true
            }
            Msg::WsReady(response) => {
                self.data = response.map(|data| data.value).ok();
                true
            }


            Msg::Render(timestamp) => {
                // Render functions are likely to get quite large, so it is good practice to split
                // it into it's own function rather than keeping it inline in the update match
                // case. This also allows for updating other UI elements that may be rendered in
                // the DOM like a framerate counter, or other overlaid textual elements.
                self.render_gl(timestamp);
                true
            }
            Msg::Add => {
                let description = self.state.value.trim();
                if !description.is_empty() {
                    let entry = Entry {
                        description: description.to_string(),
                        completed: false,
                        editing: false,
                    };
                    self.state.entries.push(entry);

                }
                self.state.value = "".to_string();
                true
            }
            Msg::Edit(idx) => {
                let edit_value = self.state.edit_value.trim().to_string();
                self.state.complete_edit(idx, edit_value);
                self.state.edit_value = "".to_string();
                true
            }
            Msg::Update(val) => {
                println!("Input: {}", val);
                self.state.value = val;
                true
            }
            Msg::UpdateEdit(val) => {
                println!("Input: {}", val);
                self.state.edit_value = val;
                true
            }
            Msg::Remove(idx) => {
                self.state.remove(idx);
                true
            }
            Msg::SetFilter(filter) => {
                self.state.filter = filter;
                true
            }
            Msg::ToggleEdit(idx) => {
                self.state.edit_value = self.state.entries[idx].description.clone();
                self.state.clear_all_edit();
                self.state.toggle_edit(idx);
                true
            }
            Msg::ToggleAll => {
                let status = !self.state.is_all_completed();
                self.state.toggle_all(status);
                true
            }
            Msg::Toggle(idx) => {
                self.state.toggle(idx);
                true
            }
            Msg::ClearCompleted => {
                self.state.clear_completed();
                true
            }
            Msg::Focus => {
                if let Some(input) = self.focus_ref.cast::<InputElement>() {
                    input.focus().unwrap();
                }
                true
            }
        }
        // self.storage.store(KEY, Json(&self.state.entries));
        // true
    }

    fn change(&mut self, _: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {

        html! {
            <div class="C0">

                <div class="C1">
                    <div class="C21">

                    </div>

                    <div class="C22">
                        <h5>{ "Frisco-Networking-Workshop" }</h5>
                    </div>
                </div>

                <div class="C1">


                    <div class="C32">
                        <nav class="menu">
                            <button onclick=self.link.callback(|_| Msg::FetchData(Format::Json, false))>
                                { "Fetch Data" }
                            </button>

                            <button onclick=self.link.callback(|_| Msg::FetchData(Format::Json, true))>
                                { "Fetch Data [binary]"}
                            </button>

                            { self.view_data() }
                            <button disabled=self.ws.is_some()
                                    onclick=self.link.callback(|_| WsAction::Connect)>
                                { "Connect To WebSocket" }
                            </button>
                            <button disabled=self.ws.is_none()
                                    onclick=self.link.callback(|_| WsAction::SendData(false))>
                                { "Send To WebSocket" }
                            </button>
                            <button disabled=self.ws.is_none()
                                    onclick=self.link.callback(|_| WsAction::SendData(true))>
                                { "Send To WebSocket [binary]" }
                            </button>
                            <button disabled=self.ws.is_none()
                                    onclick=self.link.callback(|_| WsAction::Disconnect)>
                                { "Close WebSocket connection" }
                            </button>

                            
                        </nav>


                    </div>

                    <div class="C31">
                        <div class="CCanvas">
                            <canvas ref=self.node_ref.clone() />
                        </div>
                    </div>
                </div>
            </div>
        }
    }
}

impl Model {

    fn view_data(&self) -> Html {
        if let Some(value) = self.data {
            html! {
                <p> { value } </p>
            }
        } else {
            html! {
                <p> { "tettjcc.ww." } </p>
            }
        }
    }


    fn fetch_json(&mut self, binary: AsBinary) -> yew_services::fetch::FetchTask {
        let callback = self.link.batch_callback(
            move |response: Response<Json<Result<DataFromFile, Error>>>| {
                let (meta, Json(data)) = response.into_parts();
                println!("META: {:?}, {:?}", meta, data);
                if meta.status.is_success() {
                    Some(Msg::FetchReady(data))
                } else {
                    None // FIXME: Handle this error accordingly.
                }
            },
        );
        let request = Request::get("/data.json").body(Nothing).unwrap();
        if binary {
            FetchService::fetch_binary(request, callback).unwrap()
        } else {
            FetchService::fetch(request, callback).unwrap()
        }
    }

    // pub fn fetch_toml(&mut self, binary: AsBinary) -> yew_services::fetch::FetchTask {
    //     let callback = self.link.batch_callback(
    //         move |response: Response<Toml<Result<DataFromFile, Error>>>| {
    //             let (meta, Toml(data)) = response.into_parts();
    //             println!("META: {:?}, {:?}", meta, data);
    //             if meta.status.is_success() {
    //                 Some(Msg::FetchReady(data))
    //             } else {
    //                 None // FIXME: Handle this error accordingly.
    //             }
    //         },
    //     );
    //     let request = Request::get("/data.toml").body(Nothing).unwrap();
    //     if binary {
    //         FetchService::fetch_binary(request, callback).unwrap()
    //     } else {
    //         FetchService::fetch(request, callback).unwrap()
    //     }
    // }

    fn render_gl(&mut self, timestamp: f64) {
        let gl = self.gl.as_ref().expect("GL Context not initialized!");

        let vert_code = include_str!("./basic.vert");
        let frag_code = include_str!("./basic.frag");

        // This list of vertices will draw two triangles to cover the entire canvas.
        let vertices: Vec<f32> = vec![
            -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
        ];
        let vertex_buffer = gl.create_buffer().unwrap();
        let verts = js_sys::Float32Array::from(vertices.as_slice());

        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &verts, GL::STATIC_DRAW);

        let vert_shader = gl.create_shader(GL::VERTEX_SHADER).unwrap();
        gl.shader_source(&vert_shader, &vert_code);
        gl.compile_shader(&vert_shader);

        let frag_shader = gl.create_shader(GL::FRAGMENT_SHADER).unwrap();
        gl.shader_source(&frag_shader, &frag_code);
        gl.compile_shader(&frag_shader);

        let shader_program = gl.create_program().unwrap();
        gl.attach_shader(&shader_program, &vert_shader);
        gl.attach_shader(&shader_program, &frag_shader);
        gl.link_program(&shader_program);

        gl.use_program(Some(&shader_program));

        // Attach the position vector as an attribute for the GL context.
        let position = gl.get_attrib_location(&shader_program, "a_position") as u32;
        gl.vertex_attrib_pointer_with_i32(position, 2, GL::FLOAT, false, 0, 0);
        gl.enable_vertex_attrib_array(position);

        // Attach the time as a uniform for the GL context.
        let time = gl.get_uniform_location(&shader_program, "u_time");
        gl.uniform1f(time.as_ref(), timestamp as f32);

        gl.draw_arrays(GL::TRIANGLES, 0, 6);

        let render_frame = self.link.callback(Msg::Render);
        let handle = RenderService::request_animation_frame(render_frame);

        // A reference to the new handle must be retained for the next render to run.
        self.render_loop = Some(handle);
    }


    fn view_input(&self) -> Html {
        html! {
            <input
                class="C2"
                placeholder="Frisco"
                value=&self.state.value
                oninput=self.link.callback(|e: InputData| Msg::Update(e.value))
                onkeypress=self.link.batch_callback(|e: KeyboardEvent| {
                    if e.key() == "Enter" { Some(Msg::Add) } else { None }
                })
            />

        }
    }
}

fn main() {
    yew::start_app::<Model>();
    start_websocket();
}
