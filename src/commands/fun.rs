use crate::{
    Tokens,
    commands::moderation::parse_member,
};

use std::time::Duration;

use serenity::{
    prelude::Context,
    model::misc::Mentionable,
    model::channel::{
        Message,
        ReactionType,
    },
    model::user::User,
    framework::standard::{
        Args,
        CommandResult,
        CheckResult,
        CommandError,
        macros::{
            command,
            check,
        },
    },
};
use tracing::error;
use qrcode::{
    QrCode,
    render::unicode,
};
use reqwest::{
    Client as ReqwestClient,
    Url,
};
use serde::Deserialize;
use crypto::{
    symmetriccipher,
    buffer,
    aes,
    blockmodes
};
use crypto::buffer::{
    ReadBuffer,
    WriteBuffer,
    BufferResult
};
use hex;

static KEY: [u8; 32] =  [244, 129, 85, 125, 252, 92, 208, 68, 29, 125, 160, 4, 146, 245, 193, 135, 12, 68, 162, 84, 202, 123, 90, 165, 194, 126, 12, 117, 87, 195, 9, 202];
static IV: [u8; 16] =  [41, 61, 154, 40, 255, 51, 217, 146, 228, 10, 58, 62, 217, 128, 96, 7];

#[check]
#[name = "bot_has_manage_messages"]
async fn bot_has_manage_messages_check(ctx: &Context, msg: &Message) -> CheckResult {
    let bot_id = ctx.cache.current_user().await.id.0;
    if !msg.channel(ctx)
        .await
        .unwrap()
        .guild()
        .unwrap()
        .permissions_for_user(ctx, bot_id)
        .await
        .expect("what even")
        .manage_messages()
    {
        CheckResult::new_user("I'm unable to run this command due to missing the `Manage Messages` permission.")
    } else {
        CheckResult::Success
    }
}

// Struct used to deserialize the output of the urban dictionary api call...
#[derive(Deserialize, Clone)]
struct UrbanDict {
    definition: String,
    permalink: String,
    thumbs_up: u32,
    thumbs_down: u32,
    author: String,
    written_on: String,
    example: String,
    word: String,
}

// But it returns a list, so we use this for the request.
#[derive(Deserialize)]
struct UrbanList {
    list: Vec<UrbanDict>
}

// Struct used to deserialize the response from the yandex translate api call.
#[derive(Deserialize)]
struct YandexTranslate {
    code: u16,
    lang: Option<String>,
    text: Option<Vec<String>>,
}

/// Sends a qr code of the term mentioned.
/// Usage: `.qr Hello world!`
#[command]
async fn qr(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let words = args.message();

    let code = QrCode::new(words).unwrap();
    let image = code.render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();

    msg.channel_id.say(ctx, format!(">>> ```{}```", image)).await?;
    Ok(())
}

/// Defines a term, using the urban dictionary.
/// Usage: `urban lmao`
#[command]
async fn urban(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let term = args.message();
    let url = Url::parse_with_params("http://api.urbandictionary.com/v0/define",
                                     &[("term", term)])?;

    let reqwest = ReqwestClient::new();
    let resp = reqwest.get(url)
        .send()
        .await?
        .json::<UrbanList>()
        .await?;

    if resp.list.is_empty() {
        msg.channel_id.say(ctx, format!("The term '{}' has no Urban Definitions", term)).await?;
    } else {
        let choice = &resp.list[0];
        let parsed_definition = &choice.definition.replace("[", "").replace("]", "");
        let parsed_example = &choice.example.replace("[", "").replace("]", "");
        let mut fields = vec![
            ("Definition", parsed_definition, false),
        ];
        if parsed_example != &"".to_string() {
            fields.push(("Example", parsed_example, false));
        }

        if let Err(why) = msg.channel_id.send_message(ctx, |m| {
            m.embed(|e| {
                e.title(&choice.word);
                e.url(&choice.permalink);
                e.description(format!("submitted by **{}**\n\n:thumbsup: **{}** ┇ **{}** :thumbsdown:\n", &choice.author, &choice.thumbs_up, &choice.thumbs_down));
                e.fields(fields);
                e.timestamp(choice.clone().written_on);
                e
            });
            m
        }).await {
            if "Embed too large." == why.to_string() {
                msg.channel_id.say(ctx, &choice.permalink).await?;
            } else {
                return Err(CommandError(why.to_string()));
            }
        };
    }

    Ok(())
}

/// Translates a text to the specified language.
///
/// Usage:
///
/// Translate to japanese:
/// `translate ja Hello, World!`
/// Translate from spanish to japanese:
/// `translate es-en Hola!`
#[command]
#[aliases(trans)]
#[min_args(2)]
async fn translate(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let yandex_token = {
        let data_read = ctx.data.read().await;
        let tokens = data_read.get::<Tokens>().unwrap();
        tokens["yandex"].as_str().unwrap().to_string()
    };

    let mut dest = args.single::<String>()?;
    let text = args.rest();

    dest = match dest.as_str() {
        "jp" => "ja".to_string(),
        "kr" => "ko".to_string(),
        _ => dest,
    };

    let url = Url::parse_with_params("https://translate.yandex.net/api/v1.5/tr.json/translate",
                                     &[
                                        ("key", yandex_token),
                                        ("text", text.to_string()),
                                        ("lang", dest),
                                     ])?;

    let reqwest = ReqwestClient::new();
    let resp = reqwest.get(url)
        .send()
        .await?
        .json::<YandexTranslate>()
        .await?;

    if resp.code == 200 {
        let mut fields = vec![
            ("Original Text", text.to_string() + "\n", false),
        ];

        let mut resp_langs = if let Some(l) = &resp.lang {
            l.split('-').into_iter()
        } else {
            msg.channel_id.say(ctx, "An invalid destination language was given").await?;
            return Ok(());
        };

        for translated_text in &resp.text.unwrap() {
            fields.push(("Translation", translated_text.to_string(), false));
        }


        msg.channel_id.send_message(ctx, |m| {
            m.content(format!("From **{}** to **{}**", resp_langs.next().unwrap(), resp_langs.next().unwrap()));
            m.embed(|e| {
                e.fields(fields)
            })
        }).await?;
    } else if resp.code == 404 {
        msg.channel_id.say(ctx, "The daily translation limit was exceeded.").await?;
    } else if resp.code == 413 {
        msg.channel_id.say(ctx, "The text length limit was exceeded.").await?;
    } else if resp.code == 422 {
        msg.channel_id.say(ctx, "The text could not be translated.").await?;
    } else if resp.code == 501 {
        msg.channel_id.say(ctx, "The specified target language is not supported.").await?;
    } else if resp.code == 502 {
        msg.channel_id.say(ctx, "The specified language doesn't exist.").await?;
    } else {
        msg.channel_id.say(ctx, "An unhandled error happened.").await?;
    }

    Ok(())
}

/// Searches a term on duckduckgo.com, for you.
///
/// Usage: `ddg hello world`
#[command]
#[min_args(1)]
#[aliases(ddg, duck, duckduckgo, search, better_than_google, betterthangoogle)]
async fn duck_duck_go(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let url = Url::parse_with_params("https://lmddgtfy.net/",
                                     &[("q", args.message())])?;
    msg.channel_id.say(ctx, url).await?;

    Ok(())
}

fn encrypt_bytes(data: &[u8]) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    let mut encryptor = aes::cbc_encryptor(
            aes::KeySize::KeySize256,
            &KEY,
            &IV,
            blockmodes::PkcsPadding);

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = buffer::RefReadBuffer::new(data);
    let mut buffer = [0; 4096];
    let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

    loop {
        let result = encryptor.encrypt(&mut read_buffer, &mut write_buffer, true)?;
        final_result.extend(write_buffer.take_read_buffer().take_remaining().iter().map(|&i| i));

        match result {
            BufferResult::BufferUnderflow => break,
            BufferResult::BufferOverflow => {}
        }
    }

    Ok(final_result)
}

fn decrypt_bytes(encrypted_data: &[u8]) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    let mut decryptor = aes::cbc_decryptor(
            aes::KeySize::KeySize256,
            &KEY,
            &IV,
            blockmodes::PkcsPadding);

    let mut final_result = Vec::<u8>::new();
    let mut read_buffer = buffer::RefReadBuffer::new(encrypted_data);
    let mut buffer = [0; 4096];
    let mut write_buffer = buffer::RefWriteBuffer::new(&mut buffer);

    loop {
        let result = decryptor.decrypt(&mut read_buffer, &mut write_buffer, true)?;
        final_result.extend(write_buffer.take_read_buffer().take_remaining().iter().map(|&i| i));

        match result {
            BufferResult::BufferUnderflow => break,
            BufferResult::BufferOverflow => {}
        }
    }

    Ok(final_result)
}


/// Encrypts a message.
/// You can decrypt the message with `decrypt {hex_hash}`
/// 
/// Usage: `encrypt Jaxtar is Cute!`
#[command]
#[min_args(1)]
async fn encrypt(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let message = args.message();

    let encrypted_data = encrypt_bytes(message.as_bytes()).ok().unwrap();
    let encrypted_data_text = hex::encode(encrypted_data.to_vec());

    msg.channel_id.say(ctx, format!("`{}`", encrypted_data_text)).await?;
    Ok(())
}

/// Decrypts and encrypted message.
///
/// Usage: `decrypt 36991e919634f4dc933787de47e9cb37`
#[command]
async fn decrypt(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let message = args.message();

    let encrypted_data = hex::decode(&message)?;
    let decrypted_data_bytes = match decrypt_bytes(&encrypted_data[..]) {
        Ok(ok) => ok,
        Err(why) => {
            error!("{:?}", why);
            msg.channel_id.say(ctx, format!("An invalid hash was provided. `{:?}`", why)).await?;
            return Ok(());
        },
    };

    let decrypted_data_text = String::from_utf8(decrypted_data_bytes)?;

    msg.channel_id.send_message(ctx, |m| m.embed(|e| {
        e.title(format!("From `{}`", message));
        e.description(decrypted_data_text)
    })).await?;
    Ok(())
}

async fn place_piece<'a>(board: &mut Vec<Vec<&'a str>>, user: &User, piece: &'a str, ctx: &Context) -> Result<(), ()> {
    'outer: loop {
        let mut x: Option<usize> = None;
        let mut y: Option<usize> = None;
        loop {
            if x.is_none() || y.is_none() {
                if let Some(reaction) = user.await_reaction(ctx).timeout(Duration::from_secs(120)).await {
                    let _ = reaction.as_inner_ref().delete(ctx).await;
                    let emoji = &reaction.as_inner_ref().emoji;

                    match emoji.as_data().as_str() {
                        "1\u{fe0f}\u{20e3}" => y = Some(0),
                        "2\u{fe0f}\u{20e3}" => y = Some(1),
                        "3\u{fe0f}\u{20e3}" => y = Some(2),
                        "\u{01f1e6}" => x = Some(0),
                        "\u{01f1e7}" => x = Some(1),
                        "\u{01f1e8}" => x = Some(2),
                        _ => ()
                    }
                } else {
                    return Err(());
                }
            } else {
                if !x.is_none() && !y.is_none() {
                    if board[y.unwrap()][x.unwrap()] == " " {
                        board[y.unwrap()][x.unwrap()] = piece;
                        break 'outer;
                    } else {
                        x = None;
                        y = None;
                    }
                }
            }
        }
    }

    Ok(())
}

fn check_win<'a>(board: &Vec<Vec<&'a str>>) -> Option<&'a str> {
    // diagonal \
    if board[0][0] == "O" && board[1][1] == "O" && board[2][2] == "O" {
        return Some("O");
    } else if board[0][0] == "X" && board[1][1] == "X" && board[2][2] == "X" {
        return Some("X");

    // diagonal /
    } else if board[2][0] == "O" && board[1][1] == "O" && board[0][2] == "O" {
        return Some("O");
    } else if board[2][0] == "X" && board[1][1] == "X" && board[0][2] == "X" {
        return Some("X");

    // straight lines ---
    } else if board[0] == vec!["O", "O", "O"] {
        return Some("O");
    } else if board[1] == vec!["O", "O", "O"] {
        return Some("O");
    } else if board[2] == vec!["O", "O", "O"] {
        return Some("O");
    } else if board[0] == vec!["X", "X", "X"] {
        return Some("X");
    } else if board[1] == vec!["X", "X", "X"] {
        return Some("X");
    } else if board[2] == vec!["X", "X", "X"] {
        return Some("X");

    // straigt lines |
    } else if board[0][0] == "O" && board[1][0] == "O" && board[2][0] == "O" {
        return Some("O");
    } else if board[0][1] == "O" && board[1][1] == "O" && board[2][1] == "O" {
        return Some("O");
    } else if board[0][2] == "O" && board[1][2] == "O" && board[2][2] == "O" {
        return Some("O");
    } else if board[0][0] == "X" && board[1][0] == "X" && board[2][0] == "X" {
        return Some("X");
    } else if board[0][1] == "X" && board[1][1] == "X" && board[2][1] == "X" {
        return Some("X");
    } else if board[0][2] == "X" && board[1][2] == "X" && board[2][2] == "X" {
        return Some("X");
    }


    None 
}

fn format_board(board: &Vec<Vec<&str>>) -> String {
    let mut lines = "```X | A   B   C\n--------------\n".to_string();

    for (i, x) in board.iter().enumerate() {
        let line = format!("{} | {} | {} | {}", i+1, x[0], x[1], x[2]);
        lines += format!("{}\n", line).as_str();
    }
    lines += "\nY```";
    lines
}

/// 2 player game where you must compete with the other player to be the first to obtain 3 of your pieces in line.
/// 
/// X is --- / Horizontal
/// Y is ||| / Vertical
///
/// When it's your turn, react with a number and a letter, corresponding to the position of the board.
/// If the place is taken, you will need to repick the position.
///
/// Is there an AI to play by myself? No, you have to play with another player.
///
/// Usage:
/// `ttt @timmy`
#[command]
#[aliases(ttt, tictactoe)]
#[checks("bot_has_manage_messages")]
#[min_args(1)]
async fn tic_tac_toe(mut ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user1 = &msg.author;
    let user2 = parse_member(&mut ctx, &msg, args.single::<String>()?).await?;

    let mut confirmation = msg.channel_id.say(ctx, format!("{}: Do you accept this TicTacToe match?", user2.mention())).await?;
    confirmation.react(ctx, '✅').await?;
    confirmation.react(ctx, '❌').await?;

    loop {
        if let Some(reaction) = user2.user.await_reaction(ctx).timeout(Duration::from_secs(120)).await {
            let emoji = &reaction.as_inner_ref().emoji;

            match emoji.as_data().as_str() {
                "✅" => {
                    confirmation.delete(ctx).await?;
                    break;
                },
                "❌" => {
                    confirmation.edit(ctx, |m| m.content(format!("{}: {} didn't accept the match.", user1.mention(), user2.mention()))).await?;
                    return Ok(());
                },
                _ => ()
            }
        } else {
            confirmation.edit(ctx, |m| m.content(format!("{}: {} took to long to respond.", user1.mention(), user2.mention()))).await?;
            return Ok(());
        }
    }

    let users = {
        if msg.timestamp.timestamp() % 2 == 0 {
            (user1, &user2.user)
        } else {
            (&user2.user, user1)
        }
    };

    let mut iteration = 0u8;
    let mut board = vec![
        vec![" ", " ", " "],
        vec![" ", " ", " "],
        vec![" ", " ", " "],
    ];

    let b = format_board(&board);
    let mut m = msg.channel_id.send_message(ctx, |m| {
        m.content(format!("{} (X): Select the position for your piece.", users.0.id.mention()));
        m.embed(|e| {
            e.description(&b)
        })
    }).await?;

    for i in 1..4 {
        let num = ReactionType::Unicode(String::from(format!("{}\u{fe0f}\u{20e3}", i)));
        m.react(ctx, num).await?;
    }

    let _a = ReactionType::Unicode(String::from("\u{01f1e6}"));
    let _b = ReactionType::Unicode(String::from("\u{01f1e7}"));
    let _c = ReactionType::Unicode(String::from("\u{01f1e8}"));

    m.react(ctx, _a).await?;
    m.react(ctx, _b).await?;
    m.react(ctx, _c).await?;

    loop {
        if let Err(_) = place_piece(&mut board, &users.0, "X", ctx).await {
            m.edit(ctx, |m| {
                m.content("Timeout.");
                m.embed(|e| {
                    e.description(&b)
                })
            }).await?;
        };

        let b = format_board(&board);
        m.edit(ctx, |m| {
            m.content(format!("{} (O): Select the position for your piece.", users.1.id.mention()));
            m.embed(|e| {
                e.description(&b)
            })
        }).await?;

        let won = check_win(&board);

        if iteration == 4 {
            if let Some(win) = won {
                if win == "X" {
                    m.edit(ctx, |m| {
                        m.content(format!("{} (X) won!", users.0.id.mention()));
                        m.embed(|e| {
                            e.description(&b)
                        })
                    }).await?;
                } else {
                    m.edit(ctx, |m| {
                        m.content(format!("{} (O) won!", users.1.id.mention()));
                        m.embed(|e| {
                            e.description(&b)
                        })
                    }).await?;
                }
            } else {
                m.edit(ctx, |m| {
                    m.content(format!("{} and {} tied.", users.0.id.mention(), users.1.id.mention()));
                    m.embed(|e| {
                        e.description(&b)
                    })
                }).await?;
            }
            m.delete_reactions(ctx).await?;
            break;
        } else {
            if let Some(win) = won {
                if win == "X" {
                    m.edit(ctx, |m| {
                        m.content(format!("{} (X) won!", users.0.id.mention()));
                        m.embed(|e| {
                            e.description(&b)
                        })
                    }).await?;
                } else {
                    m.edit(ctx, |m| {
                        m.content(format!("{} (O) won!", users.1.id.mention()));
                        m.embed(|e| {
                            e.description(&b)
                        })
                    }).await?;
                }
                m.delete_reactions(ctx).await?;
                break;
            }
        }

        if let Err(_) = place_piece(&mut board, &users.1, "O", ctx).await {
            m.edit(ctx, |m| {
                m.content("Timeout.");
                m.embed(|e| {
                    e.description(b)
                })
            }).await?;
            m.delete_reactions(ctx).await?;
            break;
        };

        let b = format_board(&board);
        m.edit(ctx, |m| {
            m.content(format!("{} (X): Select the position for your piece.", users.0.id.mention()));
            m.embed(|e| {
                e.description(&b)
            })
        }).await?;

        let won = check_win(&board);
        if let Some(win) = won {
            if win == "X" {
                m.edit(ctx, |m| {
                    m.content(format!("{} (X) won!", users.0.id.mention()));
                    m.embed(|e| {
                        e.description(&b)
                    })
                }).await?;
            } else {
                m.edit(ctx, |m| {
                    m.content(format!("{} (O) won!", users.1.id.mention()));
                    m.embed(|e| {
                        e.description(&b)
                    })
                }).await?;
            }
            m.delete_reactions(ctx).await?;
            break;
        }
        
        iteration += 1;
    }

    Ok(())
}
