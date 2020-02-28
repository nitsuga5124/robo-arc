/// This is a discord bot made with `serenity.rs` as a Rust learning project.
/// If you see a lot of different ways to do the same thing, specially with error handling,
/// this is indentional, as it helps me remember the concepts that rust provides, so they can be
/// used in the future for whatever they could be needed.
///
/// This is lisenced with the copyleft license Mozilla Public License Version 2.0

mod utils; // Load the utils module
mod commands; // Load the commands module
use commands::booru::*; // Import everything from the booru module.
use commands::sankaku::*; // Import everything from the sankaku booru module.
use commands::osu::*; // Import everything from the osu module.
use commands::meta::*; // Import everything from the meta module.
use commands::image_manipulation::*; // Import everything from the image manipulation module.
use utils::database::get_database;

use std::{
    collections::{
        HashSet,
        HashMap,
    },
    io::prelude::*,
    sync::Arc,
    fs::File,
};

use toml::Value;
use postgres::Client as PgClient;

use hey_listen::sync::ParallelDispatcher as Dispatcher;

use serenity::{
    utils::Colour,
    client::{
        Client,
        bridge::gateway::ShardManager,
    },
    model::{
        channel::{
            Message,
            Reaction,
            ReactionType,
        },
        gateway::{
            Ready,
            Activity,
        },
        user::OnlineStatus,
        id::{
            UserId,
            //GuildId,
        },
    },
    prelude::{
        EventHandler,
        Context,
        Mutex,
        TypeMapKey,
        RwLock,
    },
    framework::standard::{
        Args,
        CommandResult,
        CommandGroup,
        DispatchError,
        HelpOptions,
        help_commands,
        StandardFramework,
        macros::{
            group,
            help,
        },
    },
};

struct ShardManagerContainer;
struct DatabaseConnection;
struct Tokens;
struct AnnoyedChannels;
struct RecentIndex;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

impl TypeMapKey for DatabaseConnection {
    type Value = Arc<RwLock<PgClient>>;
}

impl TypeMapKey for Tokens {
    type Value = Value;
}

impl TypeMapKey for AnnoyedChannels {
    type Value = HashSet<u64>;
}

impl TypeMapKey for RecentIndex {
    type Value = HashMap<u64, usize>;
}


// The basic commands group is being defined here.
// this group includes the commands that basically every bot has, nothing really special.
#[group("Meta")]
#[description = "All the basic commands that every bot should have."]
#[commands(ping, test, react, invite, source, toggle_annoy)]
struct Meta;

// The SankakuComplex command group.
// This group will contain commands for the variants Chan and Idol of the sankaku boorus.
#[group("Sankaku")]
#[description = "All the NSFW/BSFW related commands."]
#[commands(idol)]
struct Sankaku;

#[group("osu!")]
#[description = "All the osu! related commands"]
#[commands(configure_osu, recent)]
struct Osu;

// The Booru command group.
// This group will contain every single command from every booru that gets implemented.
// As you can see on the last line, the description also supports urk markdown.
#[group("Boorus")]
#[description = "All the booru related commands.\n\
Available parameters:\n\
`-x` Explicit\n\
`-q` Questionable\n\
`-n` Non Safe (Random between E or Q)\n\
`-r` Any Rating\n\n\
Inspired by -GN's WaifuBot ([source](https://github.com/isakvik/waifubot/))"]
#[commands(safebooru)]
struct Boorus;

#[group("Image Manipulation")]
#[description = "All the image manipulaiton based commands."]
#[commands(pride)]
struct ImageManipulation;

// This is a custom help command.
// Each line has the explaination that is required.
#[help]
// This is the basic help message
// We use \ at the end of the line to easily allow for newlines visually on the code.
#[individual_command_tip = "Hello!\n\
If youd like to get extra information about a specific command, just pass it as an argument.\n\
You can also react with 🚫 on any message sent by the bot to delete it.\n"]
// This is the text that gets displayed when a given parameter was not found for information.
#[command_not_found_text = "Could not find: `{}`."]
// This is the level of similarities between the given argument and possible other arguments.
// This is used to give suggestions in case of a typo.
#[max_levenshtein_distance(3)]
// This makes it so specific sections don't get showed to the user if they don't have the
// permission to use them.
#[lacking_permissions = "Hide"]
// In the case of just lacking a role to use whatever is necessary, nothing will happen.
#[lacking_role = "Nothing"]
// In the case of being on the wrong channel type (either DM for Guild only commands or vicecersa)
// the command will be ~~striked~~
#[wrong_channel = "Strike"]
fn my_help(
    ctx: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>
) -> CommandResult {
    let mut ho = help_options.clone();
    ho.embed_error_colour = Colour::from_rgb(255, 30, 30);
    ho.embed_success_colour= Colour::from_rgb(141, 91, 255);
    help_commands::with_embeds(ctx, msg, args, &ho, groups, owners)
}



struct Handler; // Defines the handler to be used for events.

impl EventHandler for Handler {
    /// on_ready event on d.py
    /// This function triggers when the bot is ready.
    fn ready(&self, ctx: Context, ready: Ready) {
        // Changes the presence of the bot to "Listening to C++ cry a Rusted death."
        ctx.set_presence(
            Some(Activity::listening("C++ cry a Rusted death.")),
            OnlineStatus::Online
        );

        println!("{} is ready to rock!", ready.user.name);
    }

    /// on_message event on d.py
    /// This function triggers every time a message is sent.
    fn message(&self, ctx: Context, msg: Message) {
        // Ignores itself.
        if &msg.author.id.0 == ctx.cache.read().user.id.as_u64() {
            return;
        }

        let data_read = ctx.data.read();
        let annoyed_channels = data_read.get::<AnnoyedChannels>();

        if annoyed_channels.as_ref().map(|set| set.contains(&msg.channel_id.0)).unwrap_or(false) {
            if msg.content == "no u" {
                let _ = msg.reply(&ctx, "no u"); // reply pings the user
            } else if msg.content == "ayy" {
                let _ = msg.channel_id.say(&ctx, "lmao"); // say just send the message

            }
        }

        // This is an alternative way to make commands that doesn't involve the Command Framework.
        // this is not recommended as it would block the event thread, which Framework Commands
        // don't do.
        // This command just an example command made this way.
        //
        //if msg.content == ".hello" {
        //  msg.channel_id.say("Hello!")
        //}
    }

    /// on_raw_reaction_remove event on d.py
    /// This function triggers every time a reaction gets removed on a message.
    fn reaction_remove(&self, ctx: Context, add_reaction: Reaction) {
        // Ignores all reactions that come from the bot itself.
        if &add_reaction.user_id.0 == ctx.cache.read().user.id.as_u64() {
            return;
        }
        
        // Triggers reaction events from the commands.
        let dispatcher = {
            let mut context = ctx.data.write();
            context.get_mut::<DispatcherKey>().expect("Expected Dispatcher.").clone()
        };

        dispatcher.write().dispatch_event(
            &DispatchEvent::ReactEvent(add_reaction.message_id, add_reaction.emoji.clone(), false));

    }

    /// on_raw_reaction_add event on d.py
    /// This function triggers every time a reaction gets added to a message.
    fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        // Ignores all reactions that come from the bot itself.
        if &add_reaction.user_id.0 == ctx.cache.read().user.id.as_u64() {
            return;
        }

        // Triggers reaction events from the commands.
        let dispatcher = {
            let mut context = ctx.data.write();
            context.get_mut::<DispatcherKey>().expect("Expected Dispatcher.").clone()
        };

        dispatcher.write().dispatch_event(
            &DispatchEvent::ReactEvent(add_reaction.message_id, add_reaction.emoji.clone(), false));

        let msg = ctx.http.as_ref().get_message(add_reaction.channel_id.0, add_reaction.message_id.0)
            .expect("Error while obtaining message");

        let data_read = ctx.data.read();
        let annoyed_channels = data_read.get::<AnnoyedChannels>();

        let annoy = if annoyed_channels.as_ref().unwrap().contains(&msg.channel_id.0) {true} else {false};

        match add_reaction.emoji {
            // Matches custom emojis.
            ReactionType::Custom{id, ..} => {
                // If the emote is the GW version of slof, React back.
                // This also shows a couple ways to do error handling.
                if id.0 == 375459870524047361 {
                    let reaction = msg.react(&ctx, add_reaction.emoji);
                    if let Err(why) = reaction {
                        eprintln!("There was an error adding a reaction: {}", why)
                    }
                    if annoy {
                        let _ = msg.channel_id.say(&ctx, format!("<@{}>: qt", add_reaction.user_id.0));
                    }
                }
            },
            // Matches unicode emojis.
            ReactionType::Unicode(s) => {
                if annoy {
                    // This will not be kept here for long, as i see it being very annoying eventually.
                    if s == "🤔" {
                        let _ = msg.channel_id.say(&ctx, format!("<@{}>: What ya thinking so much about",
                                                                 add_reaction.user_id.0));
                    }
                } else {
                    // This makes every message sent by the bot get deleted if 🚫 is on the reactions.
                    // aka If you react with 🚫 on any message sent by the bot, it will get deleted.
                    // This is helpful for antispam and anti illegal content measures.
                    if s == "🚫" {
                        let msg = ctx.http.as_ref().get_message(add_reaction.channel_id.0, add_reaction.message_id.0)
                            .expect("Error while obtaining message");
                        if msg.author.id == ctx.cache.read().user.id {
                            let _ = msg.delete(&ctx);
                        }
                    }
                }
            },
            // Ignore the rest of the cases.
            _ => (), // complete code
            //_ => {}, // incomplete code / may be longer in the future
        }
    }
}



/// The main function!
/// Here's where everything starts.
/// This main function is a little special, as it returns Result, which allows ? to be used for
/// error handling.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Opens the config.toml file and reads it's content
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // gets the discord and osu api tokens from config.toml
    let tokens = contents.parse::<Value>().unwrap();
    let bot_token = tokens["discord"].as_str().unwrap();
    // Defines a client with the token obtained from the config.toml file.
    // This also starts up the Event Handler structure defined earlier.
    let mut client = Client::new(
        bot_token,
        Handler)?;

    // Closure to define global data.
    {
        let mut data = client.data.write();
        data.insert::<DatabaseConnection>(Arc::clone(&Arc::new(RwLock::new(get_database()?)))); // Make the database connection global.
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager)); // Make the shard manager global.
        data.insert::<Tokens>(tokens);

        let mut dispatcher: Dispatcher<DispatchEvent> = Dispatcher::default();
        dispatcher.num_threads(4).expect("Could not construct threadpool");
        data.insert::<DispatcherKey>(Arc::new(RwLock::new(dispatcher)));
        data.insert::<RecentIndex>(HashMap::new());

        {
            let db_client = Arc::clone(data.get::<DatabaseConnection>().expect("no database connection found"));
            let raw_annoyed_channels = {
                let mut db_client = db_client.write();
                db_client.query("SELECT channel_id from annoyed_channels", &[])?
            };
            let mut annoyed_channels = HashSet::new();
            
            for row in raw_annoyed_channels {
                annoyed_channels.insert(row.get::<_, i64>(0) as u64);
            }

            data.insert::<AnnoyedChannels>(annoyed_channels);
        }
    }

    &client.threadpool.set_num_threads(8);
    
    // Obtains and defines the owner/owners of the Bot Application
    // and the bot id. 
    let (owners, bot_id) = match client.cache_and_http.http.get_current_application_info() {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        },
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    // Time to configure the Command Framework!
    // This is what allows for easier and faster commaands.
    client.with_framework(StandardFramework::new() // Create a new framework
        .configure(|c| c
            .prefixes(vec![".", "arc!"]) // Add a list of prefixes to be used to invoke commands.
            .on_mention(Some(bot_id)) // Add a bot mention as a prefix.
            .with_whitespace(true) // Allow a whitespace between the prefix and the command name.
            //.dynamic_prefixes({
            //    let vec = [",,,", ",,,,"];
            //    let mut index = 0;

            //    let x = |_ctx: &mut Context, msg: &Message| {
            //        if msg.is_private() {
            //            return Some(",".to_owned());
            //        } else {
            //            let guild_id = msg.guild_id.unwrap_or(GuildId(0));
            //            if guild_id.0 == 182892283111276544 {
            //                let indexed = vec[0];
            //                let pick = Some(indexed.to_owned());
            //                index += 1;
            //                return pick;
            //        }
            //    } 
            //    return Some(",,".to_owned());
            //    };
            //    vec![x]
            //})
            .owners(owners) // Defines the owners, this can be later used to make owner specific commands.
        )

        // This is for errors that happen before command execution.
        .on_dispatch_error(|ctx, msg, error| {
            eprintln!("{:?}", error);
            match error {
                // Notify the user if the reason of the command failing to execute was because of
                // inssufficient arguments.
                DispatchError::NotEnoughArguments { min, given } => {
                    let s = format!("I need {} arguments to run this command, but i was only given {}.", min, given);

                    let _ = msg.channel_id.say(&ctx, s);
                },
                _ => eprintln!("Unhandled dispatch error."),
            }
        })
        
        // This lambda/closure function executes every time a command finishes executing.
        // It's used here to handle errors that happen in the middle of the command.
        .after(|ctx, msg, _cmd_name, error| {
            if let Err(why) = &error {
                eprintln!("{:?}", &error);
                let err = format!("{}", why.0);
                let _ = msg.channel_id.say(&ctx, &err);
            }
        })

        // Small error event that triggers when a command doesn't exist.
        .unrecognised_command(|_, _, unknown_command_name| {
            eprintln!("Could not find command named '{}'", unknown_command_name);
        })
        .group(&META_GROUP) // Load `Meta` command group
        .group(&SANKAKU_GROUP) // Load `SankakuComplex` command group
        .group(&BOORUS_GROUP) // Load `Boorus` command group
        .group(&OSU_GROUP) // Load `osu!` command group
        .group(&IMAGEMANIPULATION_GROUP) // Load `image manipulaiton` command group
        .help(&MY_HELP) // Load the custom help.
    );

    // start listening for events by starting a single shard
    if let Err(why) = client.start() {
        eprintln!("An error occurred while running the client: {:?}", why);
    }

    Ok(())
}
