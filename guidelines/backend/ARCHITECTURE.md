# Backend Architecture

## Purpose

This document defines the architecture of the listening service.
It covers the pipeline, the state machine, and the resource rules.
It applies to all backend crates.
See [[FOLDER_STRUCTURE]] for the folder layout.

## Pipeline Overview

The service runs as a background daemon.
It stays idle until the wake command occurs.
Then it listens, transcribes, resolves intent, and executes.

```
                    ┌─────────────┐
                    │    Idle     │◄──────────────────────────┐
                    └──────┬──────┘                           │
                           │ wake detected                    │
                    ┌──────▼──────┐                           │
                    │  Listening  │                           │
                    └──────┬──────┘                           │
                           │ audio buffer                     │
              ┌────────────▼────────────┐                     │
              │  Fast Transcriber       │                     │
              │  (real-time, low cost)  │                     │
              └────────────┬────────────┘                     │
                           │ interim text                     │
              ┌────────────▼────────────┐                     │
              │  Slow Transcriber       │                     │
              │  (accurate, correction) │                     │
              └────────────┬────────────┘                     │
                           │ final text                       │
              ┌────────────▼────────────┐                     │
              │  Intent Resolver        │                     │
              │  (database + plugins)   │                     │
              └────────────┬────────────┘                     │
                           │ intent + entities                │
              ┌────────────▼────────────┐                     │
              │  Executor               │                     │
              │  (plugin host)          │                     │
              └────────────┬────────────┘                     │
                           │ done / error / timeout           │
                           └─────────────────────────────────┘
```

Each stage communicates through typed events.
A stage does not call the next stage directly.
The daemon wires the stages through channels.

## State Machine

The daemon holds one state machine.
The state machine has these states.

| State | Description |
| --- | --- |
| Idle | Wait for wake word. Use minimum resources. |
| Listening | Capture audio after wake. Show timeout if no speech. |
| Transcribing | Run fast and slow transcribers. Emit interim and final text. |
| Resolving | Match text to intent. Query intent database and plugins. |
| Executing | Run the action. Return success or failure. |
| Error | Handle failure and return to Idle. |

Rules:

- The service enters Listening only from Idle.
- The service returns to Idle after Executing, after Error, or after timeout.
- The service cancels Listening when the timeout expires.
- The service cancels Transcribing when the user stops speech or the silence timeout expires.
- Each transition emits an event for the frontend.

## Event System

Use typed events for all cross-stage communication inside the backend.
Define the events in alice-core.
Do not use untyped strings or dynamic maps.

```rust
// backend/crates/alice-core/src/event.rs
pub enum SystemEvent {
    WakeDetected { timestamp: Instant },
    ListeningStarted,
    ListeningStopped { reason: StopReason },
    TranscriptInterim { text: String },
    TranscriptFinal { text: String },
    TranscriptCorrected { original: String, corrected: String },
    IntentResolved { intent: IntentId, confidence: f32 },
    IntentNotFound { text: String },
    ExecutionStarted { intent: IntentId },
    ExecutionCompleted { intent: IntentId },
    ExecutionFailed { intent: IntentId, error: CoreError },
}
```

Use `tokio::sync::broadcast` or `mpsc` channels for internal events.
The daemon subscribes to the internal channels.
The daemon also forwards the events to the frontend through sockets.

## Project Boundary

The backend and the frontend are two separate Rust projects.
They do not share code.
They communicate only through the network.

- The backend exposes a REST API and a socket stream.
  The backend crate `alice-daemon` owns the server.
- The frontend consumes the REST API and the socket stream.
  The frontend crate `alice-frontend` owns the client.
- The internal channels stay inside the backend.
  The frontend never subscribes to a backend channel directly.
  The frontend subscribes to the socket.

```
backend internal:  audio -> transcribe -> intent -> executor  (via mpsc/broadcast)
                        │
                        ▼
                   API server  ── REST ──►  frontend bridge (request-response)
                   API server  ── Socket ─► frontend bridge (event stream)
```

REST handles request-response:

- `GET /api/status` returns the daemon status.
- `GET /api/config` and `PUT /api/config` read and write configuration.
- `GET /api/plugins` and `POST /api/plugins/:id/enable` manage plugins.
- `GET /api/history` returns recent transcripts and executions.

Sockets handle streaming events:

- The frontend opens one WebSocket to `WS /api/events`.
- The backend pushes every `SystemEvent` as JSON through the socket.
- The socket is the only way the frontend receives wake, transcript, and execution events.
- The backend does not push through REST.

Rules:

- Define the REST schema and the socket message schema in the backend.
  Document them with OpenAPI or with DTO schemas in `alice-core`.
- The frontend copies the DTOs into `frontend/src/bridge/types.rs` or generates them.
  The frontend never imports `alice-core` from the backend.
- Keep one event for one purpose on the socket.
  Do not reuse an event for two purposes.
- Version the API.
  A breaking change needs a new version path such as `/api/v2/`.

## Resource Rules

The service runs silently in the background.
It must use few resources when idle.

- The wake detector runs continuously but with a small model and small buffer.
  Use a fixed-size ring buffer for audio.
  Do not grow the buffer in Idle.
- The fast transcriber starts only in Listening.
  Stop the transcriber when the state returns to Idle.
- The slow transcriber starts after the fast transcriber produces interim text.
  The slow transcriber corrects, it does not replace the fast path.
  Run the slow transcriber on a separate task to avoid blocking audio capture.
- The intent resolver and executor run only when final text is ready.
  Release their resources after execution.
- Use `tokio::select!` with cancellation tokens for timeouts.
  Cancel the Listening task when the silence timeout expires.
- Do not poll in a tight loop.
  Use async sleep or channel receive.
- Measure CPU and memory in Idle.
  Keep Idle CPU near zero and memory below the defined budget.

## Transcriber Separation

Keep fast and slow transcribers in separate modules.
They share a trait but have different implementations.

```rust
pub trait Transcriber: Send + Sync {
    fn transcribe_chunk(&mut self, audio: &[f32]) -> TranscribeResult;
    fn finalize(&mut self) -> TranscribeResult;
}
```

- The fast transcriber returns interim results with low latency.
- The slow transcriber returns a corrected result with higher latency.
- The daemon merges the results: show interim text, then replace with corrected text.
- Do not block the audio thread with the slow transcriber.

## Intent Database and Resolver

The intent resolver matches text to intent.

- The intent database holds intent definitions, examples, and required entities.
- The resolver queries the database and the plugin intents.
- The resolver returns the best match with a confidence score.
- The resolver rejects low-confidence matches and returns IntentNotFound.
- Keep the database behind a trait so tests use a fake implementation.

```rust
pub trait IntentDatabase: Send + Sync {
    fn find_candidates(&self, text: &str) -> Vec<IntentCandidate>;
    fn get_intent(&self, id: &IntentId) -> Option<IntentDefinition>;
}
```

### Resolver engines

`router` is the engine the settings page writes, and it is the engine of every turn.
It reads the message in stages: a deterministic pass, a retrieval pass over the whole catalog, a decision over the short list it keeps, and an extraction pass for the values of the intent that won.
Every stage is configured on its own and every stage has a reader that needs no model, so a catalog of any size stays readable and a server that is down costs accuracy rather than the turn.

The daemon still reads `llama`, `gliner`, and `hybrid`, which read a message with a llama.cpp server, with a built in GLiNER model, or with both.
The settings page no longer offers them, so a stage carries the reader that used to be the engine: the address and the model of the server belong to every stage that reads it, and the built in model belongs to the stage that reads spans with it.

### The layered router

A flat reader meets a wall once a catalog grows.
A span reader treats every intent as a label that competes with every other label, so a large label set costs accuracy and a long list has to be read by matching instead.
A language model asked to choose among the whole catalog needs one option per intent, so the catalog stops at 26.

Both walls come from asking one reader one flat question.
The router asks four narrow ones instead, in order, and stops at the first stage that can answer.

```
message
  │
  ├─ 1. deterministic pass   prove it from the message alone       no model
  │
  ├─ 2. retrieval pass       rank every intent, keep the top K     words, embeddings
  │
  ├─ 3. decision pass        choose one of the top K, or none       scores, model
  │
  └─ 4. extraction pass      read the values of the chosen intent   lists, spans, model
```

**Stage 1, the deterministic pass.**
The pass answers a message it can prove and nothing else.
A message is proved in two shapes: it spells an intent out (`take a screenshot` is the name of one intent and the phrase of no other), or it names one entry of one list and reads as the action of one intent (`open firefox` is the action `open` plus the entry `firefox`, and only the intent that owns the list holding `firefox` reads that way).
When two intents read the same way the message is ambiguous and the pass gives up, because running the wrong command is worse than refusing.
The pass needs no model and no server, and it never guesses.
It reads the lists of the intents whose action the message holds and of no others, so a turn never runs every script of a large catalog.

**Stage 2, the retrieval pass.**
Every intent becomes a document of its name, its description, its phrases, and the names of its entities.
The pass ranks every document against the message with BM25 and keeps the top K (`router.top_k`, default 8).
The ranking is computed once per turn and never cached: a catalog is a few hundred documents, so the whole ranking costs less than a model call, and a cached index would have to be thrown away whenever the user edits an intent.
Two kinds of evidence are folded into one term frequency, and two rules keep it honest.

- **The name of an intent always counts**; a phrase the user wrote counts only when the message shares the action of the phrase.
  Without the rule the phrase `close firefox` of one intent holds the word `firefox`, and `open firefox` would match the intent that closes things.
- **A word that carries nothing on its own is dropped** from both the document and the message, so `what is the weather` does not match every intent that holds a `the`.
  The words that say which way a command goes stay in: `turn the volume up` and `turn the volume down` differ in those words alone.
- **A word the catalog does not know does not count against a document.**
  A message names the intent and, often, the value of an entity: `what is the weather in Porto` names the city `Porto`, and no document holds that word.
  Counting it would punish the one document that should win.

The retrieval stage offers embeddings next to the words (`router.retrieve`), weighted by `router.lexical_weight` and `router.dense_weight`.
The vector of a document is cached, because the catalog changes when a user edits an intent and not when a message arrives.

A vector comes from one of two places, and `router.embed_source` names the one to read.

- `server` asks the model server the daemon already talks to, at `POST {base_url}/embeddings`.
  A server that carries no embedding model fails with a reason and the words rank the catalog alone, so the stage never depends on a model that is not there.
- `local` runs a built in ONNX model in the daemon through ONNX Runtime (`router.embed_local_model`).
  A machine that runs the daemon alone needs no second process and no port, and a model that is not on disk fails with a reason the settings page shows.

The list matcher reads from the same place (`router.list_match`), because it asks the same question of the same model.

**Stage 3, the decision pass.**

- `score` chooses the best of the short list when its score reaches `router.floor` and stays `router.margin` clear of the second best.
  Two intents that fit a message equally well are read as no match, and a message no document really answers is refused instead of run.
  This engine needs no model.
- `rerank` chooses among the short list with a built in cross-encoder (`router.rerank_model`).
  The floor and the margin still hold the ranking, so a message the catalog does not really answer is refused before a model reads it and the floor keeps one meaning across the engines.
  The reranker then orders the intents the ranking allowed, which is what a reader that sees the message and a candidate in one sequence does better than two separate vectors can.
  A model that is not on disk, or a device that cannot run it, falls back to the scores.
- `generative` sends only the short list to the language model as a lettered decision, with an option that states that none of them fits.
  The prompt stays small however large the catalog is, which is what removes the 26 intent ceiling.
  A model that does not answer falls back to the scores, so a server that is down costs accuracy rather than the turn.

**Stage 4, the extraction pass.**

- `lists` reads the values against the lists alone, so a turn needs no model at all: a closed entity and a script entity are read out of their list, and an open entity is left without a value, which makes the turn ask for it.
- `spans` lets the built in model find the span of every label of the chosen intent and lets the language model read what the built in model could not.
- `generative` reads every value of the chosen intent with the language model.

A mention is read against a list by the rules of the matcher and, when `router.list_match` asks for embeddings, by a vector scan for what the rules could not read.
The vector scan has to reach `router.list_floor`, so a list of unrelated names stays quiet instead of answering with its nearest neighbour.
A value the deterministic pass proved names an entry of a list the daemon holds, so it wins over a value another reader guessed.

**The built in models of the router.**

Two models can run in the daemon, and both are files on disk.
An embedding model turns one text into one vector, and a reranker reads a message and one candidate together and scores the pair.
The settings page names which one each stage reads, and the daemon reads only the models the current stages need.

A model is two files in one directory of `router.models_dir`: a tokenizer and one ONNX graph.
The graph of a reranker reports one score per pair and the graph of an embedding model reports one vector per token, so the engine pools the tokens into one vector per text and maps a score through a sigmoid.
The graph decides which inputs it reads, so a model that declares no token types is read without them.
The daemon downloads a model through `POST /api/v1/resolver/local/models/{id}/download` and removes it through `DELETE /api/v1/resolver/local/models/{id}`, and the reply carries the state to poll.
A download writes a part file and renames it when it is complete, so an interrupted download never counts as an installed model.
The store holds one download at a time, because a download takes the network and a second one would make both slower.

A built in model runs on the processor or on a CUDA capable graphics card (`router.local_device`), and the built in GLiNER reader shares that choice.
The daemon asks ONNX Runtime which devices the build and the machine offer, and the settings page shows only those.
A load costs about a second and one text costs a few milliseconds on a processor, so the engine keeps the session and loads a model again only when the model or the device changes.
Inference is blocking work, so it runs on the blocking pool of the runtime.

**The settings panel of the router.**

Every stage is a choice of its own, and every stage has a reader that needs no model, so a user can switch a layer off rather than lose the turn.

| Stage | Readers | Opted out |
| --- | --- | --- |
| deterministic pass | it answers or it does not | off reads every message in the stages below |
| retrieval | words, embeddings, both | words alone need no model at all |
| decision | scores, reranker, language model | scores need no model at all |
| extraction | lists, spans, language model | lists need no model at all |

A stage that cannot run its reader falls back to the cheaper reader of the same stage, so a server that is down or a model that is not on disk costs accuracy rather than the turn.
The settings page names the reader each stage would use right now, and `GET /api/v1/resolver` carries that state.

The panel draws the stages as a node graph.
A stage is a block of the chain, a place a stage reads (the model server, the built in model files, the built in GLiNER) is a block wired to every stage that reads it, and a turn that leaves the chain (a refusal, or a question for a value the intent needs) is a block of its own.
A place is drawn only while a stage reads it, so the graph never shows a server or a model file a turn would not touch, and a link that carries a chat request is not drawn next to a stage that reads a list.
The three kinds of block stand in three columns, so every link runs forward: the places a stage reads in the first column, the chain in the middle, and the turns that leave it in the last one.
A turn that leaves the chain sits on the row of the stage it leaves, so no two links cross, which the settings page keeps that way by construction.

The graph is a canvas rather than a scroll region: the first view fits the whole graph to the panel, a drag moves it, and the wheel or the controls change the zoom, so a wide graph needs no scrollbar and a reader who wants one step zooms in.
The graph is read only, and picking a block opens the settings of that block beside it, so one block is on screen at a time and the panel stays short however many stages the pipeline carries.
A stage that would fall back shows the reader it really runs instead of the one the settings name.

**The route of a router turn** is the stages the message passed, in the order they ran.
`RouteTrace` collects one `RouteStep` per stage as the router reads, and the service adds the extraction stage, which runs outside the router.
A step names the stage (`fast_path`, `retrieve`, `decide`, `extract`), how it ended (`matched`, `refused`, `passed`, `fell_back`, `skipped`), the reader that really ran (`rules`, `words`, `lexical`, `dense`, `hybrid`, `scores`, `reranker`, `model`, `lists`, `spans`), the model of that reader, one sentence about what it read, the candidates it read, and how long it took.
A stage that fell back names the reader it ran, so a turn reports the reader that answered it rather than the reader the settings picked, and the stage time says where the milliseconds of a turn went.

The stored shortcut keeps its own fields next to the route: the stage that decided (`fast_path`, `retrieve`, `rerank`, or `none`) and the short list it decided from, best first, with the evidence of each score (`words`, `embedding`, `words+embedding`, `reranker`, or `fast_path`).
A refusal carries the same list and the reason it refused, so a user who reads `no intent matched` still reads what the catalog offered and why it was refused.
The model of a turn is the model of the reader that decided it, so a reranking stage reports the built in reranker and a generative stage reports the language model.
The confidence of a reranking turn is the score of the reranker on the pair it chose, and the confidence of the other stages is the score of the ranking.

**The values of a turn** carry the reader that read each of them (`fast_path`, `list`, `embedding`, `spans`, `model`, or `lists`), the engine and the model behind that reader, what the reader read before the daemon matched it to an entry of a list, and the similarity an embedding match reached.
The service records a reader once and reads the map before and after it ran, so the values it added or replaced are the values it read, and the value that stood there before is what it read.
So a value the deterministic pass proved, a value the rules of a list matched, and a value a model guessed are told apart in the transcript, and a name a model read is kept next to the entry it was matched to.

**Reading a sentence without a turn.**
`POST /api/v1/resolver/preview` reads one message with the stored settings and runs nothing: no command, no store, and no stage on the socket stream.
Everything else is a turn, so the reply carries the metadata a stored turn carries, and the settings page shows the route of a sentence before a turn depends on it.

**The fixture benchmark.**
`backend/config/router-eval.json` holds messages with the intent the catalog holds for each of them, or null for a message the catalog must refuse.
`resolver::router::eval` runs the shipped catalog against the set with the stages that need no model and reports the exact rate, the wrong-intent rate, the abstain rate, the over-refusal rate, and how many turns each stage carried.
A scoring constant moves only with a measurement: the set is held to zero wrong intents, zero intents chosen for a message the catalog does not hold, and at least 85 percent exact.

A hybrid turn never fails on GLiNER.
When GLiNER reads no value, or when its model is not on disk, the language model reads the values instead.
An engine that cannot read at all answers the user with the way out: install the model, or switch the engine.

GLiNER reads the message and the labels in one sequence.
One label costs context, so the daemon reports a label budget.
The budget is one label per intent, one per entity, and one per value of a closed entity.
The soft limit is 20 labels and the hard limit is 30.
The settings page warns past each limit, and suggests the hybrid engine, which gives GLiNER only the labels of the chosen intent.

The llama.cpp engine names one letter per intent, so it offers at most 26 intents.
The router reads a catalog of any size, because only the short list of one turn reaches a model, and its status reports no intent limit.

A GLiNER model runs on the processor, or on a CUDA capable graphics card when the daemon is built with the `gliner-cuda` feature.
The daemon asks ONNX Runtime which devices the build and the machine offer, and the settings page shows only those.
`shell.nix` provides the CUDA libraries that the runtime loads by name.

A model is two files in `models/gliner/<id>`.
The daemon downloads them through the resolver endpoints, writes each file to a part file, and renames it when it is complete.
An interrupted download never counts as an installed model.

### The metadata of a turn

A handled turn says how the daemon read it, so a stored message explains itself after the fact.
The daemon puts one metadata object on both messages of a turn:

- the engine that chose the intent, and the engine that read the entity values
- the model each of those engines ran
- the stage of the router that decided, and the short list it decided from
- the route the message took through the router, stage by stage
- the entities of the intent with the values the resolver read and the reader that read each of them
- the command after the values were put in
- the exit code and the duration of the run

The engine of a step is the one that really read it, not the one the settings allow.
A hybrid turn reports `llama` for the intent and `gliner` for the values, or `llama` for both when GLiNER read nothing and the language model took over.
The model of a step is the one that engine ran, so a turn on a hybrid set up reports the name of the language model and the name of the built in model.
An intent that needs no value reports no value engine, so the report never names a read that did not happen.

A turn the resolver read and chose no intent for carries no intent, and it still carries the engine and the model of that read, because the resolver did the work whatever the answer.
`Resolution::Unmatched` names both, `HandlingOutcome` keeps them, and `NO_INTENT_REPLY` is the reply that goes with them.
A message no engine read at all, for example an empty intent catalog, reports nothing.

`IntentResolved` and `IntentValuesRead` carry the engine and the model of their step on the socket, so the surface names both while the turn runs.
The transcript keeps the same fields, because the store keeps them.

The store holds the object in one JSON column, `chat_message.meta`.
The shape of a turn changes with the pipeline and not with the table, and a row the daemon cannot read still reports its text and its intent.
The daemon adds the column to a database an older daemon created, so a message stored before the feature keeps its fields and reports no metadata.
Every field the daemon added later is optional in the shape, so a turn stored by an older daemon reads back with the fields it had and no route.

The route is written the same way both times a turn is reported: `chat::meta_of` reads a handled turn into the object, and the store and the preview endpoint both call it, so a sentence a user tries and a message the daemon handled carry one shape.

### Entities of an intent

An entity is `open`, `closed`, or `script`, and it is required or optional.

- An open entity takes the words of the message.
- A closed entity takes one value of the list the user configured.
- A script entity takes one value of a list a shell command answers with.

The list of a script entity belongs to the turn and not to a table: the daemon runs the script, keeps the values in memory for the configured time, and reads the value of a message out of that list.
A list inside the label budget reaches the engine as choices, like the values of a closed entity, and a longer one is read as a span that the daemon matches against the entries of the list.
A value that matches no entry leaves the entity without a value, which the turn then reports.
The script decides what a value looks like: the example `open application` runs over the desktop entries and writes the name of the program (`firefox`), because a name is what the user says and what the command of the intent runs, while a script of the user may write the path of a program instead.

A fresh installation seeds 24 example intents, which the settings page edits like any other intent.
The catalog stays inside the letters of the llama.cpp option list, so it holds one intent less than `MAX_DECISION_OPTIONS`.
A catalog past the label budget of a GLiNER model offers the surest labels — the name of each intent and the labels of its entities — and leaves the phrases to the word reader, because a phrase carries the value it acts on and so names the other intent that reads the same kind of value.
`POST /api/v1/intents/script/preview` runs a script for the settings page, so a user reads the list before a turn reads it.

The phrases of an intent are the words a user may say for it.
The resolver reads them next to the name, so a phrase that names the intent helps the engine choose.

Entities are read by two readers, because a span finder is thin at both ends of a turn.

- The model reads the message, and the words of the message read it too.
  A GLiNER model scores an action label low when the message names no value for it: it reads `open firefox` as `launch firefox` at about twelve percent, and the entity label `applications` at nothing.
  The daemon therefore scores an intent on the words its name and its phrases share with the message as well, without the articles and the prepositions that every phrase shares: `open firefox` holds `firefox` of `launch firefox` and scores that intent one half.
  The model still wins where it is surer than the words.
- The words of a message read a value of a list when the engine read no span that names one.
  The engine names the value in the words of the user, and the list may hold the value in the spelling of the machine: a script that writes `/nix/store/...-firefox-116.0.3/bin/firefox` is read by the `firefox` of the user.
  The matcher therefore reads the base name of a value and the words its base name is written with, so `blueman manager` reads `blueman-manager`.
  A mention of one or two words of the message that the name of the intent does not hold is read against the list, so `open firefox` reads `firefox` and never reads `open`.
  A value that names no entry leaves the entity without a value, so a required entity asks the user for it instead of running a command with a guess.

The two readers are why a turn on a thin model still does the common thing: `open firefox` opens Firefox, and `open obs` on a machine without obs asks for the value.

A required entity the daemon read no value for stops the turn before the command runs: the daemon asks the user for the value, because a command that misses a value does the wrong thing.
An optional entity the daemon read no value for leaves no trace in the command, because the daemon removes its placeholder before it renders the command, together with the whitespace it sat in and a quote pair that held it alone.
The next message of the conversation reaches the resolver with the asking turn as its history, so the user can answer with the values alone.

### Settings and their places

A setting belongs to a place that stores it and to a service that makes it work.
The daemon reports which of those answer, so the settings page never saves a value that cannot take effect.

- The queue and the intents are database settings.
  They need the database only.
- The llama.cpp engine needs the database and the llama.cpp server.
  The daemon reads the model list from `GET {base_url}/models`, so the user picks a name the server answers to instead of typing one.
- The built in GLiNER engine needs the database, a model on disk, and a usable device.

A control whose place does not answer is disabled and shows the reason.
The llama.cpp model becomes a disabled field with the stored value when the server is down.
The address of the server stays editable, because the user has to be able to point the daemon somewhere else.

`GET /api/v1/dependencies` reports the places.
The reply needs no database, so the page can tell a database that is down from a daemon that is down, and it carries the values the daemon starts from.

## Plugin System

The plugin host manages external capabilities.
Each plugin implements a trait.

```rust
pub trait Plugin: Send + Sync {
    fn id(&self) -> PluginId;
    fn intents(&self) -> Vec<IntentDefinition>;
    fn execute(&self, intent: &ResolvedIntent, ctx: &ExecutionContext) -> PluginResult;
}
```

Rules:

- A plugin declares its intents.
  The resolver sees plugin intents as part of the database.
- A plugin never accesses audio or transcriber internals.
- The plugin host runs plugins in isolation.
  A plugin failure does not crash the daemon.
- The plugin host validates intent parameters before execution.
  Use the schema from alice-core.
- The execution timeout applies to each plugin.
  Cancel the plugin when the timeout expires.

## Configuration

All tuning values live in alice-core config.
No crate reads environment variables directly except alice-core.
The config includes wake sensitivity, timeouts, and model paths.

See [[CONVENTIONS]] for config conventions.

## Error Handling

Each stage returns `Result<T, CoreError>`.
The stage maps internal errors to CoreError.
The daemon logs the error and returns to Idle.
The daemon never panics on expected errors.

See [[CONVENTIONS]] for error conventions.
