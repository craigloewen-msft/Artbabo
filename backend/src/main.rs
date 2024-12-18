use bevy::ecs::query::QueryFilter;
use bevy::log::Level;
use bevy::prelude::*;
use bevy::tasks::TaskPoolBuilder;
use bevy::{log::LogPlugin, tasks::TaskPool};
use bevy_eventwork::{
    AppNetworkMessage, ConnectionId, EventworkRuntime, Network, NetworkData, NetworkEvent,
    NetworkMessage,
};
use rand::rngs::StdRng;

use core::net::Ipv4Addr;
use core::num;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::ops::DerefMut;
use std::time::Duration;

use bevy::tasks::{futures_lite::future, AsyncComputeTaskPool, Task};

use reqwest::blocking::Client;

use serde_json::json;
use serde_json::Value;

use rand::seq::SliceRandom;
use rand::{thread_rng, Rng, SeedableRng};

use bevy_eventwork_mod_websockets::*;

use chrono::{DateTime, Utc};

extern crate server_responses;
use server_responses::*;

#[derive(Component)]
struct PublicRoom;

#[derive(Component)]
struct PrivateRoom;

#[derive(Component)]
struct InGame;

#[derive(Resource, Default)]
struct AzureEndpointInfo {
    image_gen_endpoint: String,
    image_gen_key: String,
    completions_endpoint: String,
    completions_key: String,
    port: u16,
}

#[derive(Resource, Default)]
struct GlobalServerValues {
    next_available_image_server_time: DateTime<Utc>,
    next_available_prompt_server_time: DateTime<Utc>,
}

#[derive(Debug, Component)]
struct ImageGenerationTask {
    task: Task<Result<String, String>>,
    prompt_data: PromptInfoDataRequest,
    status: TaskCompletionStatus,
}

#[derive(Debug, Component)]
struct CheckPromptTask {
    task: Task<Result<(), String>>,
    prompt_data: PromptInfoDataRequest,
    status: TaskCompletionStatus,
}

#[derive(Debug, Component)]
struct PromptGenerationTask {
    task: Task<Result<Vec<String>, String>>,
    status: TaskCompletionStatus,
}

#[derive(Debug, Component)]
struct HintGenerationTask {
    task: Task<Result<HashMap<u32, Vec<String>>, String>>,
    status: TaskCompletionStatus,
}

#[derive(Component, Default)]
struct RoomStateServerInfo {
    image_task_list: Vec<ImageGenerationTask>,
    prompt_task_list: Vec<CheckPromptTask>,
    prompt_generation_task_list: Vec<PromptGenerationTask>,
    hint_generation_task_list: Vec<HintGenerationTask>,
}

struct PromptInfoForHint {
    prompt: String,
    art_value: u32,
    owner_username: String,
    player_id: u32,
}

struct HintInfo {
    hint: String,
    player_id: u32,
}

#[derive(Component)]
struct GameCleanupTimer(Timer);

fn main() {
    info!("Building app");
    let mut app = App::new();

    app.add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin {
            level: Level::DEBUG,
            filter: "wgpu=error,bevy_render=info,bevy_ecs=trace".to_string(),
            custom_layer: |_| None,
        })
        .add_plugins(bevy_eventwork::EventworkPlugin::<WebSocketProvider, TaskPool>::default())
        .insert_resource(EventworkRuntime(
            TaskPoolBuilder::new().num_threads(2).build(),
        ))
        .insert_resource(NetworkSettings::default())
        .insert_resource(AzureEndpointInfo::default())
        .insert_resource(GlobalServerValues::default())
        .add_systems(Startup, setup_connections)
        .add_systems(Startup, setup_networking.after(setup_connections))
        .add_systems(Update, handle_connection_events)
        .add_systems(Update, tick_timers)
        .add_systems(Update, handle_timer_events)
        .add_systems(Update, tick_cleanup_timer)
        .add_systems(Update, handle_cleanup_timer)
        .add_systems(Update, handle_image_generation_tasks)
        .add_systems(Update, handle_check_prompt_tasks)
        .add_systems(Update, handle_prompt_generation_tasks)
        .add_systems(Update, handle_hint_generation_tasks)
        .listen_for_message::<RoomJoinRequest, WebSocketProvider>()
        .add_systems(Update, room_join_request)
        .listen_for_message::<StartGameRequest, WebSocketProvider>()
        .add_systems(Update, start_game_request)
        .listen_for_message::<PromptInfoDataRequest, WebSocketProvider>()
        .add_systems(Update, prompt_info_data_update)
        .listen_for_message::<GameActionRequest, WebSocketProvider>()
        .add_systems(Update, game_action_request_update)
        .run();
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
        .send();

    match response {
        Ok(returned_response) => {
            info!("Sent request successfully");
            info!("Response: {:?}", returned_response);

            match returned_response.json::<Value>() {
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

        let response =
            get_chat_completion(request_body, &completions_endpoint, &completions_key).await;

        // Sleep for cooloff time
        std::thread::sleep(Duration::from_secs(request_cooloff_time));

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
                std::thread::sleep(Duration::from_secs(request_cooloff_time));

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
        std::thread::sleep(Duration::from_secs(request_cooloff_time));

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
    let client = Client::new();

    let response = client
        .post(completions_endpoint)
        .header("api-key", completions_key)
        .json(&request_body)
        .send();

    let error_string;

    match response {
        Err(e) => {
            error_string = format!("Failed to send request: {:?}", e);
        }
        Ok(returned_response) => match returned_response.json::<Value>() {
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

fn send_message_to_all_players<T>(
    round_end_info: &T,
    room_state: &RoomState,
    net: &Res<Network<WebSocketProvider>>,
) -> Result<(), String>
where
    T: Clone + NetworkMessage,
{
    for player in room_state.players.iter() {
        match net.send_message(ConnectionId { id: player.id }, round_end_info.clone()) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to send message: {:?}", e);
                return Err(format!("Failed to send message: {:?}", e));
            }
        }
    }

    Ok(())
}

// fn update_prompts_for_all_players(
//     room_state: &RoomState,
//     net: &Res<Network<WebSocketProvider>>,
// ) -> Result<(), String> {
//     for player in room_state.players.iter() {
//         match net.send_message(ConnectionId { id: player.id }, player.prompt_data.clone()) {
//             Ok(_) => info!(
//                 "Sent prompt info to {} with id {}",
//                 player.username, player.id
//             ),
//             Err(e) => {
//                 error!("Failed to send message: {:?}", e);
//                 return Err(format!("Failed to send message: {:?}", e));
//             }
//         }
//     }

//     Ok(())
// }

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

fn progress_round(
    room_state: &mut RoomState,
    timer: &mut RoundTimer,
    commands: &mut Commands,
    entity: Entity,
    net: &Res<Network<WebSocketProvider>>,
) {
    info!(
        "Progressing round for room {} from {:?}",
        room_state.room_id, room_state.game_state
    );
    match room_state.game_state {
        GameState::WaitingRoom => {
            room_state.game_state = GameState::PromptGenerationWaiting;
            commands.entity(entity).insert(InGame);
        }
        GameState::PromptGenerationWaiting => {
            room_state.game_state = GameState::ImageCreation;

            // Set game timer
            timer.0 = Timer::from_seconds(10.0, TimerMode::Once);
            timer.0.pause();

            // Set clean up timer
            commands
                .entity(entity)
                .insert(GameCleanupTimer(Timer::from_seconds(
                    ((BIDDING_ROUND_TIME + BIDDING_ROUND_END_TIME)
                            * room_state.prompts_per_player as f32
                            * room_state.players.len() as f32
                        + END_SCORE_SCREEN_TIME
                        + 130.0 ) // For creating the initial images
                        * 4.0, // Add a 4 x safety to be safe
                    TimerMode::Once,
                )));
        }
        GameState::ImageCreation => {
            room_state.game_state = GameState::BiddingRound;
            room_state.setup_next_round();
            timer.0 = Timer::from_seconds(BIDDING_ROUND_TIME, TimerMode::Once);
        }
        GameState::BiddingRound => {
            room_state.game_state = GameState::BiddingRoundEnd;
            timer.0 = Timer::from_seconds(BIDDING_ROUND_END_TIME, TimerMode::Once);
            let round_end_info_option = room_state.finalize_round();

            // Send round end info to all players
            if let Some(round_end_info) = round_end_info_option {
                let _ =
                    send_message_to_all_players::<RoundEndInfo>(&round_end_info, room_state, net);
            } else {
                error!("Failed to finalize round: {:?}", room_state);
            }
        }
        GameState::BiddingRoundEnd => {
            if room_state.remaining_prompts.len() > 0 {
                room_state.game_state = GameState::BiddingRound;
                timer.0 = Timer::from_seconds(BIDDING_ROUND_TIME, TimerMode::Once);
                room_state.setup_next_round();
            } else {
                room_state.game_state = GameState::EndScoreScreen;
                timer.0 = Timer::from_seconds(END_SCORE_SCREEN_TIME, TimerMode::Once);
                let game_end_info_option = room_state.get_game_end_info();

                // Send game end info to all players
                if let Some(game_end_info) = game_end_info_option {
                    let _ =
                        send_message_to_all_players::<GameEndInfo>(&game_end_info, room_state, net);
                } else {
                    error!("Failed to finalize game: {:?}", room_state);
                }
            }
        }
        GameState::EndScoreScreen => {
            room_state.game_state = GameState::Intro;
            info!("Game ended for room {}, removing room", room_state.room_id);
            commands.entity(entity).despawn_recursive();
        }
        _ => {
            error!(
                "Progress round called but no handler found for: {:?}",
                room_state.game_state
            );
            commands.entity(entity).remove::<RoundTimer>();
        }
    }
    // TODO: Despawn and clean up everything else
}

fn find_and_handle_room<T, Q>(
    mut new_messages: EventReader<NetworkData<T>>,
    net: &Res<Network<WebSocketProvider>>,
    query: &mut Query<
        (
            Entity,
            &mut RoomState,
            &mut RoundTimer,
            &mut RoomStateServerInfo,
        ),
        Q,
    >,
    mut callback: impl FnMut(
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
        &Res<Network<WebSocketProvider>>,
        &Entity,
        &NetworkData<T>,
    ),
) where
    T: HasRoomId + Debug + Event,
    Q: QueryFilter, // Makes the filter generic, allowing flexible `Query` options
{
    for message in new_messages.read() {
        info!("Received new message: {:?}", message);
        // Get the room state entity
        for (entity, mut room_state, mut timer, mut room_state_server_info) in query.iter_mut() {
            if entity.index() == message.room_id() {
                callback(
                    &mut room_state,
                    &mut timer,
                    &mut room_state_server_info,
                    net,
                    &entity,
                    message,
                );
            }
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

// === Scene handling functions ===

// === Core functionality ===
fn setup_connections(mut azure_endpoint_info: ResMut<AzureEndpointInfo>) {
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
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| {
            error!("Warning: PORT is not set");
            String::new()
        })
        .parse()
        .unwrap_or_else(|err| {
            error!("Warning: Failed to parse PORT: {}", err);
            0 // Default to 0 or any other appropriate port number
        });

    azure_endpoint_info.image_gen_endpoint = azure_ai_image_endpoint;
    azure_endpoint_info.image_gen_key = azure_ai_image_key;
    azure_endpoint_info.completions_endpoint = azure_ai_completions_endpoint;
    azure_endpoint_info.completions_key = azure_ai_completions_key;
    azure_endpoint_info.port = port;
}

fn setup_networking(
    mut net: ResMut<Network<WebSocketProvider>>,
    settings: Res<NetworkSettings>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
    azure_endpoint_info: Res<AzureEndpointInfo>
) {
    let port = if azure_endpoint_info.port == 0 {
        8081
    } else {
        azure_endpoint_info.port
    };

    let socket_address = if DEBUG_MODE {
        SocketAddr::new(
            "127.0.0.1".parse().expect("Could not parse ip address"),
            port,
        )
    } else {
        SocketAddr::new("0.0.0.0".parse().expect("Could not parse ip address"), port)
    };

    info!("Address of the server: {}", socket_address.to_string());

    match net.listen(socket_address, &task_pool.0, &settings) {
        Ok(_) => (),
        Err(err) => {
            error!("Could not start listening: {}", err);
            panic!();
        }
    }

    info!("Started listening for new connections!");
}

fn handle_connection_events(
    mut network_events: EventReader<NetworkEvent>,
    mut query: Query<(Entity, &mut RoomState)>,
    net: Res<Network<WebSocketProvider>>,
    mut commands: Commands,
) {
    for event in network_events.read() {
        if let NetworkEvent::Connected(conn_id) = event {
            info!("New player connected: {}", conn_id);
        } else if let NetworkEvent::Disconnected(conn_id) = event {
            info!("Player disconnected: {}", conn_id);
            for (entity, mut room_state) in query.iter_mut() {
                room_state.disconnect_player(*conn_id);

                if room_state.players.len() == 0 {
                    info!("Room {} is empty, despawning", room_state.room_id);
                    commands.entity(entity).despawn_recursive();
                } else {
                    // Send updated room state to all players
                    let room_state_deref_mut = room_state.deref_mut();
                    match send_message_to_all_players::<RoomState>(
                        &room_state_deref_mut,
                        &room_state_deref_mut,
                        &net,
                    ) {
                        Ok(_) => info!(
                            "Updated player state for all players in room {}",
                            room_state.room_id
                        ),
                        Err(e) => error!("Failed to send message: {:?}", e),
                    }
                }
            }
        }
    }
}

fn tick_timers(time: Res<Time>, mut query: Query<&mut RoundTimer>) {
    for mut timer in query.iter_mut() {
        timer.0.tick(time.delta());
    }
}

fn handle_timer_events(
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoomStateServerInfo,
        &mut RoundTimer,
    )>,
    mut commands: Commands,
    net: Res<Network<WebSocketProvider>>,
) {
    for (entity, mut room_state, mut _room_state_server_info, mut timer) in query.iter_mut() {
        if timer.0.finished() {
            info!("Timer finished for room {}", room_state.room_id);
            progress_round(
                room_state.deref_mut(),
                timer.deref_mut(),
                &mut commands,
                entity,
                &net,
            );

            // Send updated room state to all players
            let room_state_deref_mut = room_state.deref_mut();
            match send_message_to_all_players::<RoomState>(
                &room_state_deref_mut,
                &room_state_deref_mut,
                &net,
            ) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        }
    }
}

fn tick_cleanup_timer(time: Res<Time>, mut query: Query<&mut GameCleanupTimer>) {
    for mut timer in query.iter_mut() {
        timer.0.tick(time.delta());
    }
}

fn handle_cleanup_timer(
    mut query: Query<(Entity, &RoomState, &GameCleanupTimer)>,
    mut commands: Commands,
) {
    for (entity, room_state, timer) in query.iter_mut() {
        if timer.0.finished() {
            info!(
                "Cleanup timer finished for room {}, removing",
                room_state.room_id
            );
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn handle_image_generation_tasks(
    mut commands: Commands,
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoomStateServerInfo,
        &mut RoundTimer,
    )>,
) {
    for (entity, mut room_state, mut room_state_server_info, mut timer) in query.iter_mut() {
        let remaining_tasks = room_state_server_info.image_task_list.len();
        let mut task_completed = false;
        for compute_task_info in room_state_server_info.image_task_list.iter_mut() {
            if let Some(string_option) =
                future::block_on(future::poll_once(&mut compute_task_info.task))
            {
                info!(
                    "Handling result of image generation task: {:?}",
                    compute_task_info
                );

                match string_option {
                    Ok(string_value) => {
                        task_completed = true;
                        info!(
                            "Image generation completed: {:?}, remaining tasks: {}",
                            string_value, remaining_tasks
                        );
                        compute_task_info.prompt_data.prompt.image_url = string_value.clone();
                        compute_task_info.status = TaskCompletionStatus::Completed;

                        // Send completed prompt to player
                        let mut return_prompt_data = compute_task_info.prompt_data.clone();
                        return_prompt_data.state = PromptState::FullyCompleted;

                        // Add the prompt to the remaining prompts
                        let completed_prompt =
                            std::mem::take(&mut compute_task_info.prompt_data.prompt);

                        room_state.remaining_prompts.push(completed_prompt);

                        info!("Sending prompt info to player: {:?}", return_prompt_data);

                        match net.send_message(
                            ConnectionId {
                                id: return_prompt_data.prompt.owner_id,
                            },
                            return_prompt_data,
                        ) {
                            Ok(_) => info!("Sent prompt info successfully",),
                            Err(e) => {
                                error!("Failed to send image generation message: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Task failed to complete: {:?}", compute_task_info);
                        compute_task_info.status = TaskCompletionStatus::Error;

                        let mut return_prompt_data = compute_task_info.prompt_data.clone();

                        // Send an error message to the player
                        return_prompt_data.error_message = e.clone();
                        return_prompt_data.state = PromptState::Error;
                        match net.send_message(
                            ConnectionId {
                                id: return_prompt_data.prompt.owner_id,
                            },
                            return_prompt_data,
                        ) {
                            Ok(_) => info!("Sent prompt info successfully",),
                            Err(e) => {
                                error!("Failed to send message: {:?}", e);
                            }
                        }
                    }
                }
            }
        }
        // Remove all finished tasks
        room_state_server_info
            .image_task_list
            .retain(|task| task.status == TaskCompletionStatus::InProgress);

        // If room is ready to go then proceed
        if task_completed {
            if check_if_room_is_prepped(&room_state) {
                info!(
                    "All tasks are completed for room {}, progressing to next round",
                    room_state.room_id
                );
                progress_round(
                    room_state.deref_mut(),
                    timer.deref_mut(),
                    &mut commands,
                    entity,
                    &net,
                );

                // Send updated room state to all players
                let room_state_deref_mut = room_state.deref_mut();
                match send_message_to_all_players::<RoomState>(
                    &room_state_deref_mut,
                    &room_state_deref_mut,
                    &net,
                ) {
                    Ok(_) => info!(
                        "Updated player state for all players in room {}",
                        room_state.room_id
                    ),
                    Err(e) => error!("Failed to send message: {:?}", e),
                }
            }
        }
    }
}

fn handle_check_prompt_tasks(
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
    azure_endpoint_info: Res<AzureEndpointInfo>,
    mut global_server_values: ResMut<GlobalServerValues>,
) {
    for (_entity, _room_state, _timer, mut room_state_server_info) in query.iter_mut() {
        if room_state_server_info.prompt_task_list.len() > 0 {
            let mut new_image_tasks = Vec::new();

            for compute_task_info in room_state_server_info.prompt_task_list.iter_mut() {
                if let Some(prompt_check_success) =
                    future::block_on(future::poll_once(&mut compute_task_info.task))
                {
                    info!(
                        "Handling result of check prompt task: {:?}",
                        compute_task_info
                    );

                    match prompt_check_success {
                        Ok(_) => {
                            info!("Prompt check completed successfully");
                            compute_task_info.status = TaskCompletionStatus::Completed;

                            // Create a task to get the image URL for each prompt
                            let thread_pool = AsyncComputeTaskPool::get();

                            let endpoint = azure_endpoint_info.image_gen_endpoint.clone();
                            let api_key = azure_endpoint_info.image_gen_key.clone();
                            let input_string =
                                compute_task_info.prompt_data.prompt.prompt_answer.clone();

                            let time_to_wait = increment_server_time(
                                &mut global_server_values.next_available_image_server_time,
                                IMAGE_GEN_TIMEOUT_SECS,
                            );

                            info!("Starting image generation task in {} seconds", time_to_wait);

                            let task = thread_pool.spawn(async move {
                                std::thread::sleep(Duration::from_secs(time_to_wait as u64));
                                get_image_url(input_string, endpoint, api_key).await
                            });

                            new_image_tasks.push(ImageGenerationTask {
                                task,
                                prompt_data: compute_task_info.prompt_data.clone(),
                                status: TaskCompletionStatus::InProgress,
                            });

                            // Send completed prompt to player
                            let mut return_prompt_data = compute_task_info.prompt_data.clone();
                            return_prompt_data.state = PromptState::PromptCompleted;

                            match net.send_message(
                                ConnectionId {
                                    id: return_prompt_data.prompt.owner_id,
                                },
                                return_prompt_data,
                            ) {
                                Ok(_) => info!("Sent prompt info successfully",),
                                Err(e) => {
                                    error!("Failed to send message: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Task failed to complete: {:?}", e);
                            compute_task_info.status = TaskCompletionStatus::Error;

                            let mut return_prompt_data = compute_task_info.prompt_data.clone();

                            // Send an error message to the player
                            return_prompt_data.error_message = e.clone();
                            return_prompt_data.state = PromptState::Error;
                            match net.send_message(
                                ConnectionId {
                                    id: return_prompt_data.prompt.owner_id,
                                },
                                return_prompt_data,
                            ) {
                                Ok(_) => info!("Sent prompt info successfully",),
                                Err(e) => {
                                    error!("Failed to send message: {:?}", e);
                                }
                            }
                        }
                    }
                }
            }

            // Add new image tasks to the list
            room_state_server_info
                .image_task_list
                .extend(new_image_tasks);

            // Remove all finished tasks
            room_state_server_info
                .prompt_task_list
                .retain(|task| task.status == TaskCompletionStatus::InProgress);
        }
    }
}

fn handle_prompt_generation_tasks(
    net: Res<Network<WebSocketProvider>>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
    azure_endpoint_info: Res<AzureEndpointInfo>,
    mut global_server_values: ResMut<GlobalServerValues>,
) {
    for (entity, mut room_state, mut timer, mut room_state_server_info) in query.iter_mut() {
        if room_state_server_info.prompt_generation_task_list.len() > 0 {
            let mut generate_hints_check = false;
            let mut prompt_list_for_hints = Vec::<PromptInfoForHint>::new();
            for compute_task_info in room_state_server_info
                .prompt_generation_task_list
                .iter_mut()
            {
                if let Some(generated_prompt_list_result) =
                    future::block_on(future::poll_once(&mut compute_task_info.task))
                {
                    info!(
                        "Handling result of prompt generation task: {:?}",
                        compute_task_info
                    );

                    match generated_prompt_list_result {
                        Err(e) => {
                            error!("Failed to generate prompts: {:?}", e);
                            compute_task_info.status = TaskCompletionStatus::Error;

                            // TODO: Handle this error
                        }
                        Ok(generated_prompt_list) => {
                            let mut player_index = 0;
                            let mut player_prompt_count = 0;

                            compute_task_info.status = TaskCompletionStatus::Completed;

                            info!("Generated prompts: {:?}", generated_prompt_list);

                            // Send out prompts to all players
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
                                match net
                                    .send_message(ConnectionId { id: player.id }, new_prompt_data)
                                {
                                    Ok(_) => info!(
                                        "Sent prompt info to {} with id {}",
                                        player.username, player.id
                                    ),
                                    Err(e) => {
                                        error!("Failed to send message: {:?}", e);
                                    }
                                }
                            }

                            generate_hints_check = true;

                            // Update room state
                            let room_state_deref_mut = room_state.deref_mut();

                            progress_round(
                                room_state_deref_mut,
                                timer.deref_mut(),
                                &mut commands,
                                entity,
                                &net,
                            );

                            match send_message_to_all_players::<RoomState>(
                                &room_state_deref_mut,
                                &room_state_deref_mut,
                                &net,
                            ) {
                                Ok(_) => info!("Started game in room {}", room_state.room_id),
                                Err(e) => error!("Failed to send message: {:?}", e),
                            }
                        }
                    }
                }
            }

            // Remove all finished tasks
            room_state_server_info
                .prompt_generation_task_list
                .retain(|task| task.status == TaskCompletionStatus::InProgress);

            if generate_hints_check {
                // Start generate hints task
                let thread_pool = AsyncComputeTaskPool::get();

                let number_of_hints =
                    room_state.players.len() as u32 * room_state.prompts_per_player;
                let azure_endpoint_url = azure_endpoint_info.completions_endpoint.clone();
                let azure_endpoint_key = azure_endpoint_info.completions_key.clone();

                let time_to_wait = increment_server_time(
                    &mut global_server_values.next_available_prompt_server_time,
                    PROMPT_GEN_TIMEOUT_SECS * number_of_hints as u64,
                );

                let mut rng = StdRng::from_entropy();

                info!("Starting hint generation task in {} seconds", time_to_wait);

                let room_state_clone = room_state.clone();

                let task = thread_pool.spawn(async move {
                    std::thread::sleep(Duration::from_secs(time_to_wait as u64));
                    generate_hints(
                        &prompt_list_for_hints,
                        &mut rng,
                        azure_endpoint_url,
                        azure_endpoint_key,
                        &room_state_clone,
                    )
                    .await
                });

                room_state_server_info
                    .hint_generation_task_list
                    .push(HintGenerationTask {
                        task,
                        status: TaskCompletionStatus::InProgress,
                    });
            }
        }
    }
}

fn handle_hint_generation_tasks(
    net: Res<Network<WebSocketProvider>>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
) {
    for (entity, mut room_state, mut timer, mut room_state_server_info) in query.iter_mut() {
        if room_state_server_info.hint_generation_task_list.len() > 0 {
            let mut task_completed = false;
            for compute_task_info in room_state_server_info.hint_generation_task_list.iter_mut() {
                if let Some(generated_hint_list_result) =
                    future::block_on(future::poll_once(&mut compute_task_info.task))
                {
                    info!(
                        "Handling result of hint generation task: {:?}",
                        compute_task_info
                    );

                    match generated_hint_list_result {
                        Err(e) => {
                            error!("Failed to generate hints: {:?}", e);
                            compute_task_info.status = TaskCompletionStatus::Error;

                            // TODO: Handle this error
                        }
                        Ok(generated_hint_list) => {
                            compute_task_info.status = TaskCompletionStatus::Completed;
                            task_completed = true;
                            info!("Generated hints: {:?}", generated_hint_list);

                            for (player_id, player_hints) in generated_hint_list.iter() {
                                let player_option = &mut room_state
                                    .players
                                    .iter_mut()
                                    .find(|player| player.id == *player_id);

                                if let Some(player) = player_option {
                                    player.hints = player_hints.clone();
                                }
                            }
                        }
                    }
                }
            }

            // Remove all finished tasks
            room_state_server_info
                .hint_generation_task_list
                .retain(|task| task.status == TaskCompletionStatus::InProgress);

            if task_completed {
                // If all image tasks are completed, then progress the round
                if check_if_room_is_prepped(&room_state) {
                    let room_state_deref_mut = room_state.deref_mut();

                    progress_round(
                        room_state_deref_mut,
                        timer.deref_mut(),
                        &mut commands,
                        entity,
                        &net,
                    );

                    match send_message_to_all_players::<RoomState>(
                        &room_state_deref_mut,
                        &room_state_deref_mut,
                        &net,
                    ) {
                        Ok(_) => info!("Started game in room {}", room_state.room_id),
                        Err(e) => error!("Failed to send message: {:?}", e),
                    }
                }
            }
        }
    }
}

// === API Requests ===
fn room_join_request(
    mut new_messages: EventReader<NetworkData<RoomJoinRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut room_query: Query<(Entity, &mut RoomState), Without<InGame>>,
    mut commands: Commands,
) {
    for new_message in new_messages.read() {
        info!("New room join request: {:?}", new_message);

        let searched_room_option = room_query
            .iter_mut()
            .find(|search_room_state| search_room_state.1.room_code == new_message.room_code);

        if let Some(mut room) = searched_room_option {
            // Room is found
            info!("Found existing room for join request");
            let room_state = room.1.deref_mut();

            room_state.players.push(Player::new(
                new_message.source().id,
                new_message.username.clone(),
            ));

            match send_message_to_all_players::<RoomState>(room_state, room_state, &net) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        } else {
            // Else create a new entity with room state
            info!("No room found creating a new one");

            let new_room_entity = commands.spawn(PublicRoom).id();

            let new_room_state = RoomState {
                room_id: new_room_entity.index(),
                players: vec![Player::new(
                    new_message.source().id,
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

            let mut inserted_timer = Timer::from_seconds(5.0, TimerMode::Once);
            inserted_timer.pause();

            commands
                .entity(new_room_entity)
                .insert(RoundTimer(inserted_timer));

            let cloned_room_state = new_room_state.clone();

            commands.entity(new_room_entity).insert(new_room_state);

            commands
                .entity(new_room_entity)
                .insert(RoomStateServerInfo::default());

            if new_message.room_code == "" {
                commands.entity(new_room_entity).insert(PublicRoom);
            } else {
                commands.entity(new_room_entity).insert(PrivateRoom);
            }

            match send_message_to_all_players::<RoomState>(
                &cloned_room_state,
                &cloned_room_state,
                &net,
            ) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    new_room_entity.index()
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        };
    }
}

fn start_game_request(
    new_messages: EventReader<NetworkData<StartGameRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<
        (
            Entity,
            &mut RoomState,
            &mut RoundTimer,
            &mut RoomStateServerInfo,
        ),
        Without<InGame>,
    >,
    mut commands: Commands,
    azure_endpoint_info: Res<AzureEndpointInfo>,
    mut global_server_values: ResMut<GlobalServerValues>,
) {
    find_and_handle_room(
        new_messages,
        &net,
        &mut query,
        |mut room_state, timer, room_state_server_info, net, entity, _message| {
            info!("New start game request: {:?}", _message);

            // Choose number of prompts per player
            if room_state.players.len() <= 3 {
                room_state.prompts_per_player = 2;
            } else if room_state.players.len() <= 5 {
                room_state.prompts_per_player = 2;
            } else {
                room_state.prompts_per_player = 1;
            }

            // Start task to generate all prompts for the game
            let thread_pool = AsyncComputeTaskPool::get();

            let number_of_prompts = room_state.players.len() as u32 * room_state.prompts_per_player;
            let azure_endpoint_url = azure_endpoint_info.completions_endpoint.clone();
            let azure_endpoint_key = azure_endpoint_info.completions_key.clone();

            let time_to_wait = increment_server_time(
                &mut global_server_values.next_available_prompt_server_time,
                PROMPT_GEN_TIMEOUT_SECS * number_of_prompts as u64,
            );

            let mut rng = StdRng::from_entropy();

            info!(
                "Starting prompt generation task in {} seconds",
                time_to_wait
            );

            let task = thread_pool.spawn(async move {
                std::thread::sleep(Duration::from_secs(time_to_wait as u64));
                generate_prompt_texts(
                    number_of_prompts,
                    &mut rng,
                    azure_endpoint_url,
                    azure_endpoint_key,
                )
                .await
            });

            room_state_server_info
                .prompt_generation_task_list
                .push(PromptGenerationTask {
                    task,
                    status: TaskCompletionStatus::InProgress,
                });

            progress_round(room_state, timer, &mut commands, *entity, net);

            let room_state_deref_mut = room_state.deref_mut();
            match send_message_to_all_players::<RoomState>(
                &room_state_deref_mut,
                &room_state_deref_mut,
                &net,
            ) {
                Ok(_) => info!("Started game in room {}", room_state.room_id),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        },
    );
}

fn prompt_info_data_update(
    new_messages: EventReader<NetworkData<PromptInfoDataRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
    azure_endpoint_info: Res<AzureEndpointInfo>,
    mut global_server_values: ResMut<GlobalServerValues>,
) {
    find_and_handle_room(
        new_messages,
        &net,
        &mut query,
        |room_state, _timer, room_state_server_info, net, _entity, message| {
            info!("Received prompt info data update: {:?}", message);
            let message_source = message.source();
            let incoming_connection_id = message_source.id;

            for player in room_state.players.iter_mut() {
                if player.id == incoming_connection_id {
                    if message.prompt.prompt_answer != "" {
                        info!("Generating image for prompt: {:?}", message.prompt);
                        // Create a task to check the prompt
                        let thread_pool = AsyncComputeTaskPool::get();

                        let prompt_text = message.prompt.prompt_text.clone();
                        let prompt_answer = message.prompt.prompt_answer.clone();
                        let azure_endpoint_url = azure_endpoint_info.completions_endpoint.clone();
                        let azure_endpoint_key = azure_endpoint_info.completions_key.clone();

                        let time_to_wait = increment_server_time(
                            &mut global_server_values.next_available_prompt_server_time,
                            PROMPT_GEN_TIMEOUT_SECS,
                        );

                        info!("Starting prompt check task in {} seconds", time_to_wait);

                        let task = thread_pool.spawn(async move {
                            std::thread::sleep(Duration::from_secs(time_to_wait as u64));
                            check_prompt_answer(
                                prompt_text,
                                prompt_answer,
                                azure_endpoint_url,
                                azure_endpoint_key,
                            )
                            .await
                        });

                        room_state_server_info
                            .prompt_task_list
                            .push(CheckPromptTask {
                                task,
                                prompt_data: message.additional_clone(),
                                status: TaskCompletionStatus::InProgress,
                            });
                    } else {
                        // Prompt is invalid send error
                        let mut return_prompt = message.additional_clone();
                        return_prompt.error_message = "Prompt is invalid".to_string();
                        return_prompt.state = PromptState::Error;
                        match net.send_message(ConnectionId { id: player.id }, return_prompt) {
                            Ok(_) => info!(
                                "Sent prompt info to {} with id {}",
                                player.username, player.id
                            ),
                            Err(e) => {
                                error!("Failed to send message: {:?}", e);
                            }
                        }
                    }
                }
            }
        },
    );
}

fn game_action_request_update(
    new_messages: EventReader<NetworkData<GameActionRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
) {
    // Find and handle room
    find_and_handle_room(
        new_messages,
        &net,
        &mut query,
        |mut room_state, mut timer, mut _room_state_server_info, net, entity, message| {
            info!("Received game action request: {:?}", message);

            // Handle the action
            match message.action {
                GameAction::Bid => {
                    let bid_result_option = room_state.player_bid(message.requestor_player_id);
                    // Extend timer by 1 second
                    if timer.0.remaining_secs() < BID_INCREASE_TIMER_START_WINDOW {
                        timer.0.set_duration(Duration::from_secs(
                            (timer.0.duration().as_secs_f32() + BID_INCREASE_TIMER_VALUE) as u64,
                        ));
                    }

                    // Send a bid notification to all players
                    if let Some(bid_result) = bid_result_option {
                        let _ = send_message_to_all_players::<GamePlayerNotificationRequest>(
                            &bid_result,
                            room_state,
                            net,
                        );
                    } else {
                        error!("Failed to process bid: {:?}", room_state);
                    }
                }
                GameAction::ForceBid => {
                    let bid_result_option = room_state
                        .player_force_bid(message.requestor_player_id, message.target_player_id);

                    if timer.0.remaining_secs() < BID_INCREASE_TIMER_START_WINDOW {
                        timer.0.set_duration(Duration::from_secs(
                            (timer.0.duration().as_secs_f32() + BID_INCREASE_TIMER_VALUE) as u64,
                        ));
                    }

                    // Send a bid notification to all players
                    if let Some(bid_result) = bid_result_option {
                        let _ = send_message_to_all_players::<GamePlayerNotificationRequest>(
                            &bid_result,
                            room_state,
                            net,
                        );
                    } else {
                        error!("Failed to process bid: {:?}", room_state);
                    }
                }
                GameAction::EndRound => {
                    progress_round(
                        room_state.deref_mut(),
                        timer.deref_mut(),
                        &mut commands,
                        *entity,
                        &net,
                    );
                }
            }

            let room_state_deref_mut = room_state.deref_mut();
            match send_message_to_all_players::<RoomState>(
                &room_state_deref_mut,
                &room_state_deref_mut,
                &net,
            ) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        },
    );
}
