use colored::Color;
use colored::Colorize;
use rand::rngs::StdRng;

use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::future::Future;
use std::ops::DerefMut;
use std::time::Duration;

use reqwest::Client;

use serde_json::json;
use serde_json::Value;

use rand::seq::SliceRandom;
use rand::{thread_rng, Rng, SeedableRng};

use chrono::{DateTime, Utc};

use env_logger::Builder;
use log::{error, info, LevelFilter};
use std::io::Write;

extern crate server_responses;
use server_responses::*;

use rocket::futures::lock::Mutex;
use rocket::tokio;
use std::sync::Arc;

use rocket::fs::{FileServer, relative};

use event_work_server::{EventWorkSendMessages, EventWorkSender, EventWorkServer, NetworkEvent};
use rocket::State;

extern crate event_work_server;
use event_work_server::*;

#[macro_use]
extern crate rocket;

#[derive(Default, Clone)]
struct AzureEndpointInfo {
    image_gen_endpoint: String,
    image_gen_key: String,
    completions_endpoint: String,
    completions_key: String,
}

#[derive(Default)]
struct GlobalServerValues {
    next_available_image_server_time: DateTime<Utc>,
    next_available_prompt_server_time: DateTime<Utc>,
    endpoint_info: AzureEndpointInfo,
}

struct PromptInfoForHint {
    prompt: String,
    art_value: u32,
    owner_username: String,
    player_id: u32,
}

#[derive(Debug, Clone)]
struct RoomList {
    rooms: HashMap<usize, RoomState>,
    id_count: usize,
}

impl RoomList {
    fn new() -> Self {
        RoomList {
            rooms: HashMap::new(),
            id_count: 0,
        }
    }

    fn room_state_insert(&mut self, mut room: RoomState) -> usize {
        self.id_count += 1;
        room.room_id = self.id_count as u32;
        let room_id = room.room_id as usize;
        self.insert(room_id, room);
        return room_id;
    }

    fn insert(&mut self, id: usize, room: RoomState) -> Option<RoomState> {
        self.rooms.insert(id, room)
    }

    fn get(&self, id: &usize) -> Option<&RoomState> {
        trace!("Getting room with id: {}", id);
        self.rooms.get(id)
    }

    fn get_mut(&mut self, id: &usize) -> Option<&mut RoomState> {
        self.rooms.get_mut(id)
    }

    fn remove(&mut self, id: &usize) -> Option<RoomState> {
        self.rooms.remove(id)
    }

    fn iter(&self) -> std::collections::hash_map::Iter<usize, RoomState> {
        self.rooms.iter()
    }

    fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<usize, RoomState> {
        self.rooms.iter_mut()
    }
}

#[get("/ws")]
fn websocket_connect<'r>(
    ws: ws::WebSocket,
    eventwork_server: &'r State<Arc<Mutex<EventWorkServer>>>,
) -> ws::Channel<'r> {
    ws.channel(move |stream| {
        Box::pin(async move {
            let server_listen_await_function_result = {
                eventwork_server
                    .lock()
                    .await
                    .handle_new_connection(stream)
                    .await
            };
            match server_listen_await_function_result {
                Ok(server_listen_function) => {
                    server_listen_function().await.unwrap();
                }
                Err(e) => {
                    eprintln!("Failed to handle connection: {}", e);
                }
            }
            Ok(())
        })
    })
}

#[launch]
async fn rocket() -> _ {
    setup_logger();

    let eventwork_server_original = EventWorkServer::default();
    eventwork_server_original.init().await;

    let global_server_values_reference = Arc::new(Mutex::new(GlobalServerValues {
        endpoint_info: get_azure_info(),
        ..Default::default()
    }));

    let eventwork_server_reference = Arc::new(Mutex::new(eventwork_server_original));
    let room_state_list_reference = Arc::new(Mutex::new(RoomList::new()));

    let mut eventwork_server = eventwork_server_reference.lock().await;

    if let Err(e) = eventwork_server
        .register_message::<RoomJoinRequest>({
            let room_state_list_reference_clone = room_state_list_reference.clone();
            Arc::new(move |sender: EventWorkSender| {
                Box::pin(room_join_request(
                    sender,
                    room_state_list_reference_clone.clone(),
                ))
            })
        })
        .await
    {
        eprintln!("Failed to register message: {}", e);
    }

    error!("Make this code shorter");

    if let Err(e) = eventwork_server
        .register_message::<StartGameRequest>({
            let room_state_list_reference_clone = room_state_list_reference.clone();
            let global_server_values_reference_clone = global_server_values_reference.clone();
            Arc::new(move |sender: EventWorkSender| {
                Box::pin(start_game_request(
                    sender,
                    room_state_list_reference_clone.clone(),
                    global_server_values_reference_clone.clone(),
                ))
            })
        })
        .await
    {
        eprintln!("Failed to register message: {}", e);
    }

    if let Err(e) = eventwork_server
        .register_message::<PromptInfoDataRequest>({
            let room_state_list_reference_clone = room_state_list_reference.clone();
            let global_server_values_reference_clone = global_server_values_reference.clone();
            Arc::new(move |sender: EventWorkSender| {
                Box::pin(prompt_info_data_update(
                    sender,
                    room_state_list_reference_clone.clone(),
                    global_server_values_reference_clone.clone(),
                ))
            })
        })
        .await
    {
        eprintln!("Failed to register message: {}", e);
    }

    if let Err(e) = eventwork_server
        .register_message::<GameActionRequest>({
            let room_state_list_reference_clone = room_state_list_reference.clone();
            Arc::new(move |sender: EventWorkSender| {
                Box::pin(game_action_request_update(
                    sender,
                    room_state_list_reference_clone.clone(),
                ))
            })
        })
        .await
    {
        eprintln!("Failed to register message: {}", e);
    }

    eventwork_server
        .on_network_event({
            let room_state_list_reference_clone = room_state_list_reference.clone();
            let eventwork_server_reference_clone = eventwork_server_reference.clone();
            Arc::new(move |network_event: NetworkEvent| {
                Box::pin(handle_connection_events(
                    network_event,
                    room_state_list_reference_clone.clone(),
                    eventwork_server_reference_clone.clone(),
                ))
            })
        })
        .await;

    rocket::build()
        .manage(eventwork_server_reference.clone())
        .mount("/ws", routes![websocket_connect])
        .mount("/", FileServer::from(relative!("static")))
}

// === Helper Functions ===

async fn get_image_url(
    input_string: String,
    url: String,
    api_key: String,
) -> Result<String, String> {
    // Simulate a long-running task
    info!("Starting image generation task");

    if DEBUG_MODE {
        // SLeep a random time
        // let sleep_time = rand::random::<u64>() % 1;
        // std::thread::sleep(std::time::Duration::from_secs(sleep_time));

        let random_image_list = vec![
            // "https://dalleproduse.blob.core.windows.net/private/images/4756af2f-c07e-40b9-abff-06184957db4a/generated_00.png?se=2024-11-30T22%3A05%3A37Z&sig=e2W8tJT6DwB3JY10VSV%2BR8mP2SHkKH4oWawoNbe8gvU%3D&ske=2024-12-06T07%3A57%3A19Z&skoid=09ba021e-c417-441c-b203-c81e5dcd7b7f&sks=b&skt=2024-11-29T07%3A57%3A19Z&sktid=33e01921-4d64-4f8c-a055-5bdaffd5e33d&skv=2020-10-02&sp=r&spr=https&sr=b&sv=2020-10-02",
            "https://i.ebayimg.com/images/g/JPUAAOSw0n5lBnhv/s-l1200.jpg",
            "https://picsum.photos/id/674/300/300",
            "https://picsum.photos/id/675/300/300",
            "https://picsum.photos/id/676/300/300",
            "https://picsum.photos/id/677/300/300",
            "https://picsum.photos/id/678/300/300",
        ];

        return Ok(random_image_list
            .choose(&mut thread_rng())
            .unwrap()
            .to_string());
    }

    let client = Client::new();

    let request_body = json!({
       "prompt": input_string,
        "n": 1,
        "size": "1024x1024"
    });

    let response = client
        .post(url)
        .header("api-key", api_key)
        .json(&request_body)
        .send()
        .await;

    match response {
        Ok(returned_response) => {
            info!("Sent request successfully");
            info!("Response: {:?}", returned_response);

            match returned_response.json::<Value>().await {
                Ok(json) => match json.get("data") {
                    Some(data) => match data.get(0) {
                        Some(data_first_element) => match data_first_element.get("url") {
                            Some(url) => {
                                info!("Got url: {}", url);
                                match url.as_str() {
                                    Some(url) => return Ok(url.to_string()),
                                    None => {
                                        error!("Failed to get url");
                                        return Err("Failed to get url".to_string());
                                    }
                                }
                            }
                            None => {
                                error!("Failed to get url");
                                return Err("Failed to get url".to_string());
                            }
                        },
                        None => {
                            error!("Failed to get data");
                            return Err("Failed to get data".to_string());
                        }
                    },
                    None => {
                        error!("Failed to get data {:?}", json);
                        return Err(format!("Failed to get data {}", json).to_string());
                    }
                },
                Err(e) => {
                    error!("Failed to get json: {:?}", e);
                    return Err("Failed to get json".to_string());
                }
            }
        }
        Err(e) => {
            error!("Failed to send request: {:?}", e);
            return Err("Failed to send request".to_string());
        }
    }
}

async fn check_prompt_answer(
    prompt_text: String,
    prompt_answer: String,
    completions_endpoint: String,
    completions_key: String,
) -> Result<(), String> {
    info!("Checking prompt answer");

    if DEBUG_MODE {
        return Ok(());
    }

    let request_body = json!({
       "messages": [
           {
               "role": "system",
               "content": r###"You are an AI agent who helps approve or reject prompts for a game.You are shown the given prompt, and the user's answer.
You should reject any prompts that are using words that are synonyms to any words in the input prompt, or are too close to them, like the game taboo.
These prompts will be used to generate an image, so reject prompts that use direct synonyms while accepting prompts that use descriptions."###.to_string()
           },
           {
            "role": "user",
            "content": r###"Prompt: A labrador with antlers
    Response: A dog with hooves and horns"###,
           },
           {
            "role": "assistant",
            "content": "Response is rejected. 'Dog' is too close to 'labrador' and 'horns' is too close to 'antlers'",
           },
           {
            "role": "user",
            "content": r###"Prompt: A caterpillar with a sword
    Response: Three green circles attached together with bug eyes and lots of legs, and one of the legs is holding a pointed piece of metal"###,
           },
           {
            "role": "assistant",
            "content": "Response is approved.",
           },
           {
            "role": "user",
            "content": r###"Prompt: Can of spinach
    Response: A circular metal object with a label on it. The label has a white background, and on the foreground is a green plant."###,
           },
           {
            "role": "assistant",
            "content": "Response is approved.",
           },
           {
            "role": "user",
            "content": r###"Prompt: Lightning striking a ferris wheel
    Response: At the top of the image are clouds. They are dark and seem like they are stormy. Beneath them is an amusement park, with different rides and attractions. One circular ride has a bolt of light connecting it to the heavens."###,
           },
           {
            "role": "assistant",
            "content": "Response is rejected. 'Bolt of light' is too similar to 'lightning'.",
           },
           {
            "role": "user",
            "content": format!(r###"Prompt: {}
    Response: {}"###, prompt_text, prompt_answer)
           }
       ],
       "temperature": 0.01,
    });

    let response = get_chat_completion(request_body, &completions_endpoint, &completions_key).await;

    match response {
        Ok(ai_response) => {
            if ai_response.contains("Response is approved") {
                return Ok(());
            } else {
                return Err(ai_response);
            }
        }
        Err(e) => {
            return Err(e);
        }
    }
}

async fn generate_prompt_texts(
    num_prompts: u32,
    rng: &mut StdRng,
    completions_endpoint: String,
    completions_key: String,
) -> Result<Vec<String>, String> {
    let request_cooloff_time = PROMPT_GEN_TIMEOUT_SECS;

    info!("Generating prompt texts");

    // Get a third of the prompt number rounded down
    let num_prompts_third = num_prompts / 3;

    // Get unique prompts for these three
    let mut unique_prompts: Vec<String> = Vec::new();

    for _i in 0..num_prompts_third {
        // Generate a random unique prompt and add it

        let request_body = json!({
        "messages": [
            {
                "role": "system",
                "content": r###"You are an AI agent who provides prompt ideas for a game of taboo. A user will ask for a prompt and you will provide a short one.
        Prompts can be kind of whacky, but should describe something you can make an image from.."###.to_string()
            },
            {
                "role": "user",
                "content": "Can you make me a prompt?".to_string()
            },
            {
                "role": "assistant",
                "content": "A labrador with antlers".to_string()
            },
            {
                "role": "user",
                "content": "Can you make me a prompt?".to_string()
            },
            {
                "role": "assistant",
                "content": "Lightning hitting a popsicle".to_string()
            },
            {
                "role": "user",
                "content": "Can you make me a prompt?".to_string()
            },
        ]
         });

        info!("Getting chat completion");
        let response =
            get_chat_completion(request_body, &completions_endpoint, &completions_key).await;

        // Sleep for cooloff time
        info!("Sleeping for cooloff time");
        tokio::time::sleep(Duration::from_secs(request_cooloff_time)).await;

        match response {
            Ok(ai_response) => {
                unique_prompts.push(ai_response);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    // Get the remaining number of prompts to generate
    let remaining_prompts_count = num_prompts - num_prompts_third;

    let mut similar_prompts: Vec<String> = Vec::new();

    for _i in 0..remaining_prompts_count {
        // Choose a random unique prompt
        match unique_prompts.choose(rng) {
            None => {
                error!("Failed to choose prompt");
                return Err("Failed to choose prompt".to_string());
            }
            Some(prompt) => {
                // Generate a similar prompt based on the chosen prompt

                let request_body = json!({
                "messages": [
                    {
                        "role": "system",
                        "content": r###"You are an AI agent who provides a similar prompt idea for a game of visual taboo.
                Your job is to provide another prompt that would create an image that would be visually similar, to make it hard for a user to guess which image came from which prmopt."###.to_string()
                    },
                    {
                        "role": "user",
                        "content": "Can you make me a prompt similar to: A dog with antlers".to_string()
                    },
                    {
                        "role": "assistant",
                        "content": "A fuzzy deer".to_string()
                    },
                    {
                        "role": "user",
                        "content": "Can you make me a prompt similar to: Lightning hitting a popsicle".to_string()
                    },
                    {
                        "role": "assistant",
                        "content": "Electric lollipop".to_string()
                    },
                    {
                        "role": "user",
                        "content": format!("Can you make me a prompt similar to: {}", prompt)
                    },
                ]
                 });

                let response =
                    get_chat_completion(request_body, &completions_endpoint, &completions_key)
                        .await;

                // Sleep for cooloff time
                tokio::time::sleep(Duration::from_secs(request_cooloff_time)).await;

                match response {
                    Ok(ai_response) => {
                        similar_prompts.push(ai_response);
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        };
    }

    unique_prompts.extend(similar_prompts);

    // Shuffle the prompts
    info!("Shuffling prompts");
    unique_prompts.shuffle(&mut thread_rng());

    return Ok(unique_prompts);
}

async fn generate_hints(
    prompt_info_list: &Vec<PromptInfoForHint>,
    rng: &mut StdRng,
    completions_endpoint: String,
    completions_key: String,
    room_state: &RoomState,
) -> Result<HashMap<u32, Vec<String>>, String> {
    let mut hints_list = HashMap::<u32, Vec<(String, u32)>>::new();
    let mut generated_hints_list = Vec::<(String, u32)>::new();
    let request_cooloff_time = PROMPT_GEN_TIMEOUT_SECS;

    // Get a list of strings representing the prompts
    let mut prompt_strings = Vec::<String>::new();

    for prompt_info in prompt_info_list.iter() {
        prompt_strings.push(format!(
            "{} has a prompt '{}' for a value of: {}",
            prompt_info.owner_username, prompt_info.prompt, prompt_info.art_value
        ));
    }

    info!("Prompt strings: {:?}", prompt_strings);

    // Get hints for these based on the prompts and values
    for i in 0..prompt_info_list.len() {
        let prompt_string = &prompt_strings[i];
        let prompt_info = &prompt_info_list[i];

        let request_body = json!({
        "messages": [
            {
                "role": "system",
                "content": r###"You are an AI agent who provides a hint based on a username and prompt for a game.
        Your job is to provide a somewhat vague hint for the content of the prompt and the username. Values of "###.to_string() + format!("{} are high and values of {} are low.", MIN_ART_VALUE, MAX_ART_VALUE).as_str()
            },
            {
                "role": "user",
                "content": "Billbo has a prompt 'dog with antlers' for a value of: 320".to_string()
            },
            {
                "role": "assistant",
                "content": "An image that has something to do with a pointy thing has a very low value".to_string()
            },
            {
                "role": "user",
                "content": "Monkey Man has a prompt 'lightning hitting a popsicle' for a value of: 3600".to_string()
            },
            {
                "role": "assistant",
                "content": "An electric prompt has a very high value".to_string()
            },
            {
                "role": "user",
                "content": prompt_string.clone()
            },
        ]
         });

        let response =
            get_chat_completion(request_body, &completions_endpoint, &completions_key).await;

        // Sleep for cooloff time
        tokio::time::sleep(Duration::from_secs(request_cooloff_time)).await;

        match response {
            Ok(ai_response) => {
                generated_hints_list.push((ai_response, prompt_info.player_id));
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    let mut available_players: Vec<u32> =
        room_state.players.iter().map(|player| player.id).collect();

    // Assign everyone the generated hints
    for (hint, prompt_writer_id) in generated_hints_list.iter() {
        let possible_players: Vec<u32> = available_players
            .iter()
            .filter(|&&player_id| player_id != *prompt_writer_id)
            .cloned()
            .collect();

        let random_player_id = match possible_players.choose(rng) {
            Some(player_id) => *player_id,
            None => {
                // Clone the data we need to work with, so we can safely mutate the HashMap later.
                let hint_player_id_and_index =
                    hints_list.iter().find_map(|(player_id, hint_list)| {
                        hint_list
                            .iter()
                            .enumerate()
                            .find(|(_, (_, writer_id))| writer_id != prompt_writer_id)
                            .map(|(index, _)| (*player_id, index)) // Clone `player_id` here.
                    });

                if let Some((hint_player_id, hint_index)) = hint_player_id_and_index {
                    // Extract the hint to be removed.
                    let removed_hint = {
                        let hint_list = hints_list.get(&hint_player_id).unwrap();
                        hint_list[hint_index].clone()
                    };

                    // Perform the mutation now that there are no immutable borrows.
                    hints_list
                        .get_mut(&hint_player_id)
                        .unwrap()
                        .remove(hint_index);

                    // Add the hint to the destination.
                    hints_list
                        .entry(*prompt_writer_id)
                        .or_insert_with(Vec::new)
                        .push(removed_hint);

                    // Return the player ID of the removed hint's owner.
                    hint_player_id
                } else {
                    return Err("No valid players available for hint assignment".to_string());
                }
            }
        };

        // Insert the formatted hint into the hashmap
        let hints_list_entry = hints_list.entry(random_player_id).or_insert(Vec::new());

        hints_list_entry.push((hint.clone(), *prompt_writer_id));

        // If the player has enough hints, remove them from the available players
        if hints_list_entry.len() >= room_state.prompts_per_player as usize {
            available_players.retain(|&player_id| player_id != random_player_id);
        }
    }

    // Return the hints list by mapping the values to a vector of strings only (drop the u32)
    let return_hints_list = hints_list
        .iter()
        .map(|(id, hint_list)| {
            (
                *id,
                hint_list.iter().map(|(hint, _)| hint.clone()).collect(),
            )
        })
        .collect();

    Ok(return_hints_list)
}

async fn get_chat_completion(
    request_body: Value,
    completions_endpoint: &String,
    completions_key: &String,
) -> Result<String, String> {
    info!("Made client");
    let client = Client::new();

    info!("Sending client request");
    let response = client
        .post(completions_endpoint)
        .header("api-key", completions_key)
        .json(&request_body)
        .send()
        .await;

    info!("Getting response back");

    let error_string;

    match response {
        Err(e) => {
            error_string = format!("Failed to send request: {:?}", e);
        }
        Ok(returned_response) => match returned_response.json::<Value>().await {
            Err(e) => {
                error_string = format!("Failed to get json: {:?}", e);
            }
            Ok(json) => match json.get("choices") {
                None => error_string = format!("Failed to get completions choices: {:?}", json),
                Some(choices) => match choices.get(0) {
                    None => {
                        error_string = "Failed to get first element of choices".to_string();
                    }
                    Some(data_first_element) => match data_first_element.get("message") {
                        None => {
                            error_string = "Failed to get message".to_string();
                        }
                        Some(message) => match message.get("content") {
                            None => {
                                error_string = "Failed to get message content".to_string();
                            }
                            Some(content) => match content.as_str() {
                                Some(content) => {
                                    return Ok(content.to_string());
                                }
                                None => {
                                    error_string = "Failed to get content as string".to_string();
                                }
                            },
                        },
                    },
                },
            },
        },
    }

    error!("Failed to get chat completion: {:?}", error_string);
    return Err(error_string);
}

async fn send_message_to_all_players<T, N>(
    message: &T,
    room_state: &RoomState,
    net: &N,
) -> Result<(), String>
where
    T: Clone + NetworkMessage,
    N: EventWorkSendMessages,
{
    for player in room_state.players.iter() {
        match net.send_message(player.id as usize, message.clone()).await {
            Ok(_) => {}
            Err(e) => {
                error!("Non-fatal error: Failed to send message: {:?}", e);
            }
        }
    }

    Ok(())
}

fn check_if_room_is_prepped(room_state: &RoomState) -> bool {
    if room_state.players.len() == 0 {
        return false;
    }

    if (room_state.get_completed_prompt_count()
        == room_state.players.len() as u32 * room_state.prompts_per_player)
        && room_state
            .players
            .iter()
            .all(|player| player.hints.len() == 2)
    {
        info!("Room {} is prepped", room_state.room_id);
        return true;
    } else {
        info!("Room {} is not prepped", room_state.room_id);
        return false;
    }
}

fn create_round_timer_task(
    room_id: usize,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    net_reference: Arc<Mutex<EventWorkSender>>,
    sleep_time: u64,
) {
    info!(
        "Creating round timer task for room {} with sleep time {}",
        room_id, sleep_time
    );
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(sleep_time)).await;
        // Try and find room, if it exists then progress round
        let mut room_state_list = room_state_list_reference.lock().await;
        if let Some(room_state) = room_state_list.get_mut(&room_id) {
            let room_state_clone = room_state.clone();

            progress_round(
                room_state,
                room_state_list_reference.clone(),
                net_reference.clone(),
            )
            .await;

            let net = net_reference.lock().await;

            match send_message_to_all_players::<RoomState, EventWorkSender>(
                &room_state_clone,
                &room_state_clone,
                &net,
            )
            .await
            {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        } else {
            error!("Failed to find room {} to progress round", room_id);
        }
    });
}

async fn progress_round(
    room_state: &mut RoomState,
    room_state_list_reference: Arc<Mutex<RoomList>>, // If you lock on this it will cause a deadlock
    net_reference: Arc<Mutex<EventWorkSender>>,
) {
    info!(
        "Progressing round for room {} from {:?}",
        room_state.room_id, room_state.game_state
    );
    match room_state.game_state {
        GameState::WaitingRoom => {
            room_state.game_state = GameState::PromptGenerationWaiting;
        }
        GameState::PromptGenerationWaiting => {
            room_state.game_state = GameState::ImageCreation;
        }
        GameState::ImageCreation => {
            room_state.game_state = GameState::BiddingRound;
            room_state.setup_next_round();

            create_round_timer_task(
                room_state.room_id as usize,
                room_state_list_reference,
                net_reference,
                BIDDING_ROUND_TIME,
            );
        }
        GameState::BiddingRound => {
            room_state.game_state = GameState::BiddingRoundEnd;
            let round_end_info_option = room_state.finalize_round();

            create_round_timer_task(
                room_state.room_id as usize,
                room_state_list_reference,
                net_reference.clone(),
                BIDDING_ROUND_END_TIME,
            );

            let net = net_reference.lock().await;

            // Send round end info to all players
            if let Some(round_end_info) = round_end_info_option {
                let _ = send_message_to_all_players::<RoundEndInfo, EventWorkSender>(
                    &round_end_info,
                    room_state,
                    &net,
                )
                .await;
            } else {
                error!("Failed to finalize round: {:?}", room_state);
            }
        }
        GameState::BiddingRoundEnd => {
            if room_state.remaining_prompts.len() > 0 {
                room_state.game_state = GameState::BiddingRound;
                room_state.setup_next_round();
                create_round_timer_task(
                    room_state.room_id as usize,
                    room_state_list_reference,
                    net_reference.clone(),
                    BIDDING_ROUND_TIME,
                );
            } else {
                room_state.game_state = GameState::EndScoreScreen;
                let game_end_info_option = room_state.get_game_end_info();

                create_round_timer_task(
                    room_state.room_id as usize,
                    room_state_list_reference,
                    net_reference.clone(),
                    END_SCORE_SCREEN_TIME,
                );

                let net = net_reference.lock().await;

                // Send game end info to all players
                if let Some(game_end_info) = game_end_info_option {
                    let _ = send_message_to_all_players::<GameEndInfo, EventWorkSender>(
                        &game_end_info,
                        room_state,
                        &net,
                    )
                    .await;
                } else {
                    error!("Failed to finalize game: {:?}", room_state);
                }
            }
        }
        GameState::EndScoreScreen => {
            room_state.game_state = GameState::Intro;
            info!("Game ended for room {}, removing room", room_state.room_id);
            let room_to_delete_id = room_state.room_id as usize;
            let room_state_list_reference_clone = room_state_list_reference.clone();
            tokio::spawn(async move {
                room_state_list_reference_clone
                    .lock()
                    .await
                    .remove(&room_to_delete_id);
            });
        }
        _ => {
            error!(
                "Progress round called but no handler found for: {:?}",
                room_state.game_state
            );
        }
    }
}

fn increment_server_time(server_time: &mut DateTime<Utc>, time_to_increment: u64) -> i64 {
    if DEBUG_MODE {
        return 0;
    }

    if *server_time < Utc::now() {
        *server_time = Utc::now();
    }

    let time_to_wait = (*server_time - Utc::now()).num_seconds();

    *server_time = *server_time + Duration::from_secs(time_to_increment);

    return time_to_wait;
}

fn setup_logger() {
    let mut builder = Builder::from_default_env();

    builder
        .format(|buf, record| {
            let color = match record.level() {
                log::Level::Error => Color::Red,
                log::Level::Warn => Color::Yellow,
                log::Level::Info => Color::Blue,
                log::Level::Debug => Color::Green,
                log::Level::Trace => Color::Magenta,
            };

            let file_path = record.file().unwrap_or("unknown");

            let relative_file_path = if file_path.starts_with('/') {
                file_path
                    .strip_prefix("/home/craig/dev/Artbabo/")
                    .unwrap_or("")
            } else {
                file_path
            };

            let line = if relative_file_path != "" {
                format!("{}", record.line().unwrap_or(0))
            } else {
                "".to_string()
            };

            writeln!(
                buf,
                "{}",
                format!(
                    "{}:{} [{}] - {}",
                    relative_file_path,
                    line,
                    record.level(),
                    record.args()
                )
                .color(color)
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
}

// === Core functionality ===
fn get_azure_info() -> AzureEndpointInfo {
    dotenv::dotenv().ok();
    let azure_ai_image_key = env::var("AZURE_AI_IMAGE_KEY").unwrap_or_else(|_| {
        error!("Warning: AZURE_AI_IMAGE_KEY is not set");
        String::new()
    });
    let azure_ai_image_endpoint = env::var("AZURE_AI_IMAGE_ENDPOINT").unwrap_or_else(|_| {
        error!("Warning: AZURE_AI_IMAGE_ENDPOINT is not set");
        String::new()
    });
    let azure_ai_completions_key = env::var("AZURE_AI_COMPLETIONS_KEY").unwrap_or_else(|_| {
        error!("Warning: AZURE_AI_COMPLETIONS_KEY is not set");
        String::new()
    });
    let azure_ai_completions_endpoint =
        env::var("AZURE_AI_COMPLETIONS_ENDPOINT").unwrap_or_else(|_| {
            error!("Warning: AZURE_AI_COMPLETIONS_ENDPOINT is not set");
            String::new()
        });

    let azure_endpoint_info = AzureEndpointInfo {
        image_gen_endpoint: azure_ai_image_endpoint,
        image_gen_key: azure_ai_image_key,
        completions_endpoint: azure_ai_completions_endpoint,
        completions_key: azure_ai_completions_key,
    };

    azure_endpoint_info
}

async fn handle_connection_events(
    event: NetworkEvent,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    net_reference: Arc<Mutex<EventWorkServer>>,
) -> Result<(), String> {
    if let NetworkEvent::Connected(conn_id) = event {
        info!("New player connected: {}", conn_id);
    } else if let NetworkEvent::Disconnected(conn_id) = event {
        info!("Player disconnected: {}", conn_id);

        // Get room which has this player
        let (room_id, room_state_clone) = {
            let mut room_state_list = room_state_list_reference.lock().await;
            let room_state_with_player_option =
                room_state_list.iter_mut().find(|(_room_id, room_state)| {
                    room_state
                        .players
                        .iter()
                        .any(|player| player.id == conn_id.id)
                });

            let (room_id, room_state) = match room_state_with_player_option {
                Some((room_id, room_state)) => (room_id, room_state),
                None => {
                    return Err(format!("Failed to find room with player: {}", conn_id));
                }
            };

            // Remove player from room
            room_state.players.retain(|player| player.id != conn_id.id);

            (room_id.clone(), room_state.clone())
        };

        if room_state_clone.players.len() == 0 {
            info!("Room {} is empty, despawning", room_state_clone.room_id);
            let mut room_state_list = room_state_list_reference.lock().await;
            room_state_list.remove(&room_id);
        } else {
            let net = net_reference.lock().await;

            match send_message_to_all_players::<RoomState, EventWorkServer>(
                &room_state_clone,
                &room_state_clone,
                &net,
            )
            .await
            {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state_clone.room_id
                ),
                Err(e) => return Err(format!("Failed to send message: {:?}", e)),
            }
        }
    }
    Ok(())
}

// === Long running tasks ===
async fn generate_image_task(
    time_to_wait: i64,
    mut prompt_info_data_request: PromptInfoDataRequest,
    image_gen_endpoint: String,
    image_gen_key: String,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    net_reference: Arc<Mutex<EventWorkSender>>,
) -> Result<(), String> {
    info!("Starting image gen task in {} seconds", time_to_wait);
    tokio::time::sleep(Duration::from_secs(time_to_wait as u64)).await;

    let image_url_result = get_image_url(
        prompt_info_data_request.prompt.prompt_answer.clone(),
        image_gen_endpoint,
        image_gen_key,
    )
    .await;

    match image_url_result {
        Ok(image_url) => {
            prompt_info_data_request.state = PromptState::FullyCompleted;
            prompt_info_data_request.prompt.image_url = image_url;

            // let completed_prompt = std::mem::take(&mut prompt_info_data_request.prompt);

            let mut room_state_list = room_state_list_reference.lock().await;

            let (_room_id, room_state) =
                match room_state_list.iter_mut().find(|(_room_id, room_state)| {
                    room_state.room_id == prompt_info_data_request.room_id
                }) {
                    Some(room_info) => room_info,
                    None => return Err("Couldn't find prompt room".to_string()),
                };

            room_state
                .remaining_prompts
                .push(prompt_info_data_request.prompt.clone());

            info!(
                "Sending prompt info to player: {:?}",
                prompt_info_data_request.prompt.owner_id
            );

            info!("Sent prompt info successfully");
            if check_if_room_is_prepped(room_state) {
                info!(
                    "All tasks are completed for room {}, moving to new round",
                    room_state.room_id
                );

                progress_round(
                    room_state,
                    room_state_list_reference.clone(),
                    net_reference.clone(),
                )
                .await;

                let net = net_reference.lock().await;

                send_message_to_all_players::<RoomState, EventWorkSender>(
                    room_state, room_state, &net,
                )
                .await?;
            } else {
                // Send complete message back to player
                let net = net_reference.lock().await;

                net.send_message(
                    prompt_info_data_request.prompt.owner_id as usize,
                    prompt_info_data_request,
                )
                .await?;
            }
        }
        Err(e) => {
            info!("Image gen task failed to complete: {:?}", e);

            prompt_info_data_request.state = PromptState::Error;
            prompt_info_data_request.error_message = e.clone();

            let net = net_reference.lock().await;

            net.send_message(
                prompt_info_data_request.prompt.owner_id as usize,
                prompt_info_data_request,
            )
            .await?;
            info!("Sent prompt info back successfully");
        }
    }

    Ok(())
}

async fn check_prompt_answer_task(
    time_to_wait: i64,
    mut prompt_info_data_request: PromptInfoDataRequest,
    azure_endpoint_url: String,
    azure_endpoint_key: String,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    global_server_values_reference: Arc<Mutex<GlobalServerValues>>,
    net_reference: Arc<Mutex<EventWorkSender>>,
) -> Result<(), String> {
    info!(
        "Starting check prompt answer task in {} seconds",
        time_to_wait
    );
    tokio::time::sleep(Duration::from_secs(time_to_wait as u64)).await;

    let prompt_check_success = check_prompt_answer(
        prompt_info_data_request.prompt.prompt_text.clone(),
        prompt_info_data_request.prompt.prompt_answer.clone(),
        azure_endpoint_url,
        azure_endpoint_key,
    )
    .await;

    let net = net_reference.lock().await;
    match prompt_check_success {
        Ok(_) => {
            prompt_info_data_request.state = PromptState::PromptCompleted;

            match net
                .send_message(
                    prompt_info_data_request.prompt.owner_id as usize,
                    prompt_info_data_request.clone(),
                )
                .await
            {
                Ok(_) => info!("Sent prompt info successfully"),
                Err(e) => return Err(format!("Failed to send message: {:?}", e)),
            }

            let mut global_server_values = global_server_values_reference.lock().await;

            let image_gen_time_to_wait = increment_server_time(
                &mut global_server_values.next_available_image_server_time,
                IMAGE_GEN_TIMEOUT_SECS,
            );

            report_errors_on_long_task(generate_image_task(
                image_gen_time_to_wait,
                prompt_info_data_request,
                global_server_values
                    .endpoint_info
                    .image_gen_endpoint
                    .clone(),
                global_server_values.endpoint_info.image_gen_key.clone(),
                room_state_list_reference.clone(),
                net_reference.clone(),
            ))
            .await;
        }
        Err(e) => {
            prompt_info_data_request.state = PromptState::Error;
            prompt_info_data_request.error_message = e.clone();

            net.send_message(
                prompt_info_data_request.prompt.owner_id as usize,
                prompt_info_data_request,
            )
            .await?;
        }
    }

    Ok(())
}

async fn hint_generation_task(
    time_to_wait: i64,
    mut rng: StdRng,
    prompt_list_for_hints: Vec<PromptInfoForHint>,
    azure_endpoint_url: String,
    azure_endpoint_key: String,
    room_state_clone: RoomState,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    net_reference: Arc<Mutex<EventWorkSender>>,
) -> Result<(), String> {
    info!("Starting hint generation task in {} seconds", time_to_wait);
    tokio::time::sleep(Duration::from_secs(time_to_wait as u64)).await;
    let mut generated_hint_list = generate_hints(
        &prompt_list_for_hints,
        &mut rng,
        azure_endpoint_url,
        azure_endpoint_key,
        &room_state_clone,
    )
    .await?;

    let mut room_state_list = room_state_list_reference.lock().await;

    let mut room_state = match room_state_list.get_mut(&(room_state_clone.room_id as usize)) {
        Some(room_state) => room_state,
        None => {
            return Err(format!(
                "Failed to find room with id: {}",
                room_state_clone.room_id
            ));
        }
    };

    // Update hints for all players
    for (player_id, player_hints) in generated_hint_list.iter_mut() {
        let player_option = room_state
            .players
            .iter_mut()
            .find(|player| player.id == *player_id);

        if let Some(player) = player_option {
            player.hints = player_hints.clone();
        }
    }

    if check_if_room_is_prepped(&room_state) {
        progress_round(
            &mut room_state,
            room_state_list_reference.clone(),
            net_reference.clone(),
        )
        .await;

        let net = net_reference.lock().await;
        match send_message_to_all_players::<RoomState, EventWorkSender>(
            room_state, room_state, &net,
        )
        .await
        {
            Ok(_) => info!("Started game in room {}", room_state.room_id),
            Err(e) => return Err(format!("Failed to send message: {:?}", e)),
        }
    }

    Ok(())
}

async fn prompt_generation_task(
    time_to_wait: i64,
    number_of_prompts: u32,
    mut rng: StdRng,
    azure_endpoint_url: String,
    azure_endpoint_key: String,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    room_state_index: usize,
    net_reference: Arc<Mutex<EventWorkSender>>,
    global_server_values_reference: Arc<Mutex<GlobalServerValues>>,
) -> Result<(), String> {
    info!(
        "Starting prompt generation task in {} seconds",
        time_to_wait
    );
    tokio::time::sleep(Duration::from_secs(time_to_wait as u64)).await;
    let generated_prompt_list_result = generate_prompt_texts(
        number_of_prompts,
        &mut rng,
        azure_endpoint_url.clone(),
        azure_endpoint_key.clone(),
    )
    .await;

    info!("Generated prompt texts");

    let generated_prompt_list = match generated_prompt_list_result {
        Ok(prompt_list) => prompt_list,
        Err(e) => return Err(format!("Failed to generate prompts: {:?}", e)),
    };

    let mut player_index = 0;
    let mut player_prompt_count = 0;

    info!("Generated prompts: {:?}", generated_prompt_list);

    let room_state_list_reference_clone = room_state_list_reference.clone();
    let net_reference_clone = net_reference.clone();

    let mut room_state_list = room_state_list_reference.lock().await;

    let room_state = match room_state_list.get_mut(&room_state_index) {
        Some(room_state) => room_state,
        None => {
            return Err(format!("Failed to find room with id: {}", room_state_index));
        }
    };

    // Send out prompts to all players
    let prompt_list_for_hints = {
        let net = net_reference.lock().await;

        let mut prompt_list_for_hints = Vec::<PromptInfoForHint>::new();

        for prompt_text in generated_prompt_list.iter() {
            let player = &room_state.players[player_index];

            let new_prompt = PromptInfoData {
                prompt_text: prompt_text.clone(),
                prompt_answer: String::default(),
                image_url: String::default(),
                owner_id: player.id,
                art_value: thread_rng().gen_range(MIN_ART_VALUE..MAX_ART_VALUE),
            };
            let new_prompt_data = PromptInfoDataRequest {
                prompt: new_prompt,
                room_id: room_state.room_id,
                front_end_prompt_index: None,
                state: PromptState::Proposed,
                error_message: String::default(),
            };

            prompt_list_for_hints.push(PromptInfoForHint {
                prompt: new_prompt_data.prompt.prompt_text.clone(),
                art_value: new_prompt_data.prompt.art_value.clone(),
                owner_username: player.username.clone(),
                player_id: player.id.clone(),
            });

            // Progress index counters
            player_prompt_count += 1;
            if player_prompt_count >= room_state.prompts_per_player {
                player_index += 1;
                player_prompt_count = 0;
            }

            // Send out prompt
            match net.send_message(player.id as usize, new_prompt_data).await {
                Ok(_) => info!(
                    "Sent prompt info to {} with id {}",
                    player.username, player.id
                ),
                Err(e) => {
                    error!("Failed to send message: {:?}", e);
                }
            }
        }
        prompt_list_for_hints
    };

    info!("Progressing round");
    progress_round(
        room_state,
        room_state_list_reference_clone,
        net_reference_clone,
    )
    .await;

    info!("Finished progressing round");

    let net = net_reference.lock().await;

    match send_message_to_all_players::<RoomState, EventWorkSender>(room_state, room_state, &net)
        .await
    {
        Ok(_) => info!("Started game in room {}", room_state.room_id),
        Err(e) => error!("Failed to send message: {:?}", e),
    }

    // Start hint generation task work
    let number_of_hints = room_state.players.len() as u32 * room_state.prompts_per_player;

    let room_state_clone = room_state.clone();

    let global_server_values_reference_clone = global_server_values_reference.clone();
    let mut global_server_values = global_server_values_reference_clone.lock().await;
    let hint_time_to_wait = increment_server_time(
        &mut global_server_values.next_available_prompt_server_time,
        PROMPT_GEN_TIMEOUT_SECS * number_of_hints as u64,
    );

    report_errors_on_long_task(hint_generation_task(
        hint_time_to_wait,
        rng,
        prompt_list_for_hints,
        azure_endpoint_url,
        azure_endpoint_key,
        room_state_clone,
        room_state_list_reference.clone(),
        net_reference.clone(),
    ))
    .await;

    Ok(())
}

async fn report_errors_on_long_task<F>(future: F)
where
    F: Future<Output = Result<(), String>> + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(async {
        match future.await {
            Ok(_) => {}
            Err(e) => {
                error!("Long task failed with: {:?}", e);
            }
        }
    });
}

// === API Requests ===
async fn room_join_request(
    net: EventWorkSender,
    room_state_list_reference: Arc<Mutex<RoomList>>,
) -> Result<(), String> {
    let new_message = match net.get_network_data::<RoomJoinRequest>() {
        Ok(message) => message,
        Err(e) => {
            return Err(format!("Failed to get network data: {:?}", e));
        }
    };

    info!("New room join request: {:?}", new_message);

    let mut room_state_list = room_state_list_reference.lock().await;

    let searched_room_option = room_state_list
        .iter_mut()
        .find(|search_room_state| search_room_state.1.room_code == new_message.room_code);

    if let Some(mut room) = searched_room_option {
        // Room is found
        info!("Found existing room for join request");
        let room_state = room.1.deref_mut();

        room_state.players.push(Player::new(
            net.connection_id as u32,
            new_message.username.clone(),
        ));

        match send_message_to_all_players::<RoomState, EventWorkSender>(
            room_state, room_state, &net,
        )
        .await
        {
            Ok(_) => info!(
                "Updated player state for all players in room {}",
                room_state.room_id
            ),
            Err(e) => error!("Failed to send message: {:?}", e),
        }
    } else {
        // Else create a new entity with room state
        info!("No room found creating a new one");

        let new_room_state = RoomState {
            room_id: 0,
            players: vec![Player::new(
                net.connection_id as u32,
                new_message.username.clone(),
            )],
            game_state: GameState::WaitingRoom,
            current_art_bid: ArtBidInfo::default(),
            prompts_per_player: 100,
            remaining_prompts: vec![],
            used_prompts: vec![],
            room_code: new_message.room_code.clone(),
            version_number: GAME_VERSION,
        };

        let room_id = room_state_list.room_state_insert(new_room_state);

        let room_state = match room_state_list.get(&room_id) {
            Some(room_state) => room_state,
            None => {
                return Err(format!(
                    "Couldn't find newly inserted room state: {}",
                    room_id
                ));
            }
        };

        info!("Sending room state to all players");
        match send_message_to_all_players::<RoomState, EventWorkSender>(
            &room_state,
            &room_state,
            &net,
        )
        .await
        {
            Ok(_) => info!(
                "Updated player state for all players in room {}",
                room_state.room_id
            ),
            Err(e) => error!("Failed to send message: {:?}", e),
        }
    };

    Ok(())
}

async fn start_game_request(
    net: EventWorkSender,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    global_server_values_reference: Arc<Mutex<GlobalServerValues>>,
) -> Result<(), String> {
    let new_message = match net.get_network_data::<StartGameRequest>() {
        Ok(message) => message,
        Err(e) => {
            return Err(format!("Failed to get network data: {:?}", e));
        }
    };

    let net_reference = Arc::new(Mutex::new(net));

    // Get number of prompts without keeping room_state_list_reference locked
    let (number_of_prompts, room_id) = {
        let mut room_state_list = room_state_list_reference.lock().await;
        // Find room where id matches the connection id
        let searched_room_option = room_state_list
            .iter_mut()
            .find(|(_room_id, search_room_state)| search_room_state.room_id == new_message.room_id);
        let (room_id, room_state) = match searched_room_option {
            Some(room_info) => room_info,
            None => {
                return Err(format!(
                    "Failed to find room with id: {}",
                    new_message.room_id
                ));
            }
        };

        // Choose number of prompts per player
        if room_state.players.len() <= 3 {
            room_state.prompts_per_player = 2;
        } else if room_state.players.len() <= 5 {
            room_state.prompts_per_player = 2;
        } else {
            room_state.prompts_per_player = 1;
        }

        progress_round(
            room_state,
            room_state_list_reference.clone(),
            net_reference.clone(),
        )
        .await;

        let net_clone = net_reference.lock().await;

        match send_message_to_all_players::<RoomState, EventWorkSender>(
            room_state, room_state, &net_clone,
        )
        .await
        {
            Ok(_) => info!("Started game in room {}", room_state.room_id),
            Err(e) => error!("Failed to send message: {:?}", e),
        }

        (
            room_state.players.len() as u32 * room_state.prompts_per_player,
            room_id.clone(),
        )
    };

    // Prep data for generate prompt task
    let (time_to_wait, azure_endpoint_url, azure_endpoint_key) = {
        let mut global_server_values = global_server_values_reference.lock().await;
        (
            increment_server_time(
                &mut global_server_values.next_available_prompt_server_time,
                PROMPT_GEN_TIMEOUT_SECS * number_of_prompts as u64,
            ),
            global_server_values
                .endpoint_info
                .completions_endpoint
                .clone(),
            global_server_values.endpoint_info.completions_key.clone(),
        )
    };

    let rng = StdRng::from_entropy();

    info!(
        "Starting prompt generation task in {} seconds",
        time_to_wait
    );

    report_errors_on_long_task(prompt_generation_task(
        time_to_wait,
        number_of_prompts,
        rng,
        azure_endpoint_url,
        azure_endpoint_key,
        room_state_list_reference.clone(),
        room_id,
        net_reference.clone(),
        global_server_values_reference.clone(),
    ))
    .await;

    Ok(())
}

async fn prompt_info_data_update(
    net: EventWorkSender,
    room_state_list_reference: Arc<Mutex<RoomList>>,
    global_server_values_reference: Arc<Mutex<GlobalServerValues>>,
) -> Result<(), String> {
    let message = match net.get_network_data::<PromptInfoDataRequest>() {
        Ok(message) => message,
        Err(e) => {
            return Err(format!("Failed to get network data: {:?}", e));
        }
    };

    info!("Received prompt info data update: {:?}", message);

    let incoming_connection_id = net.connection_id;

    if message.prompt.prompt_answer == "" {
        // Prompt is invalid send error
        let mut return_prompt = message.additional_clone();
        return_prompt.error_message = "Prompt is invalid".to_string();
        return_prompt.state = PromptState::Error;

        let room_state_list = room_state_list_reference.lock().await;
        let player_room_state_option = room_state_list.iter().find(|(_room_id, room_state)| {
            room_state
                .players
                .iter()
                .any(|player| player.id == incoming_connection_id as u32)
        });

        let player = match player_room_state_option {
            Some((_room_id, room_state)) => {
                match room_state
                    .players
                    .iter()
                    .find(|player| player.id == incoming_connection_id as u32)
                {
                    Some(player) => player,
                    None => {
                        return Err(format!(
                            "Failed to find player with id: {}",
                            incoming_connection_id
                        ));
                    }
                }
            }
            None => {
                return Err(format!(
                    "Failed to find player with id: {}",
                    incoming_connection_id
                ));
            }
        };

        match net.send_message(player.id as usize, return_prompt).await {
            Ok(_) => info!(
                "Sent prompt info to {} with id {}",
                player.username, player.id
            ),
            Err(e) => {
                error!("Failed to send message: {:?}", e);
            }
        }
        return Ok(());
    }

    info!("Generating image for prompt: {:?}", message.prompt);
    // Create a task to check the prompt
    let (time_to_wait, azure_endpoint_url, azure_endpoint_key) = {
        let mut global_server_values = global_server_values_reference.lock().await;
        (
            increment_server_time(
                &mut global_server_values.next_available_prompt_server_time,
                PROMPT_GEN_TIMEOUT_SECS,
            ),
            global_server_values
                .endpoint_info
                .completions_endpoint
                .clone(),
            global_server_values.endpoint_info.completions_key.clone(),
        )
    };

    let net_reference = Arc::new(Mutex::new(net));

    report_errors_on_long_task(check_prompt_answer_task(
        time_to_wait,
        message,
        azure_endpoint_url,
        azure_endpoint_key,
        room_state_list_reference.clone(),
        global_server_values_reference.clone(),
        net_reference,
    ))
    .await;

    Ok(())
}

async fn game_action_request_update(
    net: EventWorkSender,
    room_state_list_reference: Arc<Mutex<RoomList>>,
) -> Result<(), String> {
    let message = match net.get_network_data::<GameActionRequest>() {
        Ok(message) => message,
        Err(e) => {
            return Err(format!("Failed to get network data: {:?}", e));
        }
    };

    let mut room_state_list = room_state_list_reference.lock().await;

    let room_state = match room_state_list.iter_mut().find(|(_room_id, room_state)| {
        room_state
            .players
            .iter()
            .any(|player| player.id == message.requestor_player_id)
    }) {
        Some((_room_id, room_state)) => room_state,
        None => {
            return Err(format!(
                "Failed to find room with player: {}",
                message.requestor_player_id
            ));
        }
    };

    // Handle the action
    let net_reference = Arc::new(Mutex::new(net));
    let net_reference_clone = net_reference.clone();
    match message.action {
        GameAction::Bid => {
            let bid_result_option = room_state.player_bid(message.requestor_player_id);
            // Extend timer by 1 second
            // if timer.0.remaining_secs() < BID_INCREASE_TIMER_START_WINDOW {
            //     timer.0.set_duration(Duration::from_secs(
            //         (timer.0.duration().as_secs_f32() + BID_INCREASE_TIMER_VALUE) as u64,
            //     ));
            // }
            error!("TODO: Increase timer by 1 second");

            let net_reference_clone = net_reference.clone();
            let net_clone = net_reference_clone.lock().await;

            // Send a bid notification to all players
            if let Some(bid_result) = bid_result_option {
                match send_message_to_all_players::<GamePlayerNotificationRequest, EventWorkSender>(
                    &bid_result,
                    room_state,
                    &net_clone,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => error!("Failed to send message: {:?}", e),
                }
            } else {
                error!("Failed to process bid: {:?}", room_state);
            }
        }
        GameAction::ForceBid => {
            let bid_result_option =
                room_state.player_force_bid(message.requestor_player_id, message.target_player_id);

            // if timer.0.remaining_secs() < BID_INCREASE_TIMER_START_WINDOW {
            //     timer.0.set_duration(Duration::from_secs(
            //         (timer.0.duration().as_secs_f32() + BID_INCREASE_TIMER_VALUE) as u64,
            //     ));
            // }
            error!("TODO: Increase timer by 1 second");

            let net_reference_clone = net_reference.clone();
            let net_clone = net_reference_clone.lock().await;

            // Send a bid notification to all players
            if let Some(bid_result) = bid_result_option {
                match send_message_to_all_players::<GamePlayerNotificationRequest, EventWorkSender>(
                    &bid_result,
                    room_state,
                    &net_clone,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => error!("Failed to send message: {:?}", e),
                }
            } else {
                error!("Failed to process bid: {:?}", room_state);
            }
        }
        GameAction::EndRound => {
            progress_round(room_state, room_state_list_reference.clone(), net_reference).await;
        }
    }

    let net_clone = net_reference_clone.lock().await;
    match send_message_to_all_players::<RoomState, EventWorkSender>(
        room_state, room_state, &net_clone,
    )
    .await
    {
        Ok(_) => info!(
            "Updated player state for all players in room {}",
            room_state.room_id
        ),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
    Ok(())
}
