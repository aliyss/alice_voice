/**
 * A fake llama.cpp server for the e2e suite.
 *
 * The intent resolver asks a real OpenAI compatible server for one option
 * of the intent list. A real model is slow, needs a GPU, and answers
 * differently on every run, so the suite starts this server instead. It
 * reads the options out of the prompt and answers with the letter of the
 * option it is asked to choose, in the streaming shape of llama.cpp.
 *
 * The chunks are spread over time on purpose: a turn that answers in one
 * tick would hide the streamed stages of the interface from the test.
 */
import { createServer } from 'node:http';

/** The port the server listens on. */
const PORT = Number(process.env.FAKE_LLAMA_PORT ?? 8792);

/** The label of the option the server chooses when it is offered. */
const TARGET_LABEL = 'e2e weather';

/** The label of the intent that names an entity in its command. */
const TEMPLATE_LABEL = 'e2e templated';

/** The label of the intent with a script entity that names no value. */
const REQUIRED_LABEL = 'e2e required';

/** The label of the intent whose entity the words of a message read. */
const LIST_LABEL = 'e2e list';

/** The label of the option that states that nothing fits. */
const NONE_LABEL = 'none of these';

/** The reason the server reports for its choice. */
const REASON = 'the state asks for the weather';

/** The name of the schema of a request that reads entity values. */
const SCHEMA_NAME_VALUES = 'entity_values';

/** How long the server waits between two chunks, in milliseconds. */
const CHUNK_DELAY_MS = 350;

/** Build one server sent chunk in the shape llama.cpp uses. */
function chunk(delta, logprobs) {
  const choices = [{ index: 0, delta }];
  if (logprobs) {
    choices[0].logprobs = logprobs;
  }
  return `data: ${JSON.stringify({ choices, object: 'chat.completion.chunk' })}\n\n`;
}

/** Read the body of one request. */
function readBody(body) {
  try {
    return JSON.parse(body);
  } catch {
    return {};
  }
}

/** Read the prompt of one request. */
function readPrompt(body) {
  try {
    return JSON.parse(body)
      .messages.map((message) => message.content ?? '')
      .join('\n\n');
  } catch {
    return '';
  }
}

/** Read the letter of the option with the given label, or null. */
function letterOf(prompt, label) {
  const found = prompt.match(new RegExp(`^([A-Z])\\. ${label}\\b`, 'm'));
  return found ? found[1] : null;
}

/**
 * The text that asks the server to state that nothing fits.
 *
 * A real model answers that no intent fits when the message asks for
 * something the catalog does not cover. This server always picks the
 * intent of the suite when the catalog offers it, so the suite asks for
 * the `none of these` answer with a marker of its own.
 */
const NOTHING_MARKER = 'nothing fits';

/**
 * Choose the letter the model would answer with.
 *
 * The server picks the intent of the suite when the prompt offers it and
 * states that nothing fits when it does not. That keeps the answer of the
 * suite the same whether or not a test stored an intent.
 */
function chooseLetter(prompt) {
  if (prompt.includes(NOTHING_MARKER)) {
    return letterOf(prompt, NONE_LABEL) ?? 'A';
  }
  return (
    letterOf(prompt, TARGET_LABEL) ??
    letterOf(prompt, TEMPLATE_LABEL) ??
    letterOf(prompt, REQUIRED_LABEL) ??
    letterOf(prompt, LIST_LABEL) ??
    letterOf(prompt, NONE_LABEL) ??
    'A'
  );
}

/**
 * Read the schema one request asks the answer to follow.
 *
 * The resolver asks two questions of the model: which intent fits the
 * message, and which value every entity of that intent takes. The name of
 * the schema tells the two apart.
 */
function readSchema(request) {
  return request.response_format?.json_schema ?? {};
}

/**
 * Read the values of the entities out of the schema of the request.
 *
 * A closed entity offers its values as an enum, so the server answers the
 * first one. An open entity takes the words the user said, so the server
 * answers a name the suite can read back: `fake-<entity>`.
 */
function readEntityAnswer(schema) {
  const properties = schema.schema?.properties ?? {};
  const answer = {};
  for (const [name, property] of Object.entries(properties)) {
    const offered = Array.isArray(property.enum) ? property.enum : [];
    answer[name] = offered.length > 0 ? offered[0] : `fake-${name}`;
  }
  return JSON.stringify(answer);
}

/** Read the answer the model would write to one request. */
function readAnswer(body, prompt) {
  const schema = readSchema(body);
  if (schema.name === SCHEMA_NAME_VALUES) {
    return { content: readEntityAnswer(schema), letter: null };
  }
  const letter = chooseLetter(prompt);
  return {
    content: JSON.stringify({ choice: letter, reason: REASON }),
    letter,
  };
}

/** Build the chunks of one answer. */
function buildChunks({ content, letter }) {
  const head = content.slice(0, Math.floor(content.length / 2));
  const tail = content.slice(Math.floor(content.length / 2));
  const chunks = [
    chunk({ role: 'assistant', content: null }),
    chunk({ content: head }),
  ];

  // The distribution of the chosen letter is the confidence of the
  // choice, so only a choice carries log probabilities.
  if (letter === null) {
    chunks.push(chunk({ content: tail }));
  } else {
    chunks.push(
      chunk(
        { content: tail },
        {
          content: [
            {
              token: letter,
              logprob: -0.01,
              top_logprobs: [
                { token: letter, logprob: -0.01 },
                { token: 'Z', logprob: -4.6 },
              ],
            },
          ],
        },
      ),
    );
  }

  chunks.push('data: [DONE]\n\n');
  return chunks;
}

/** Wait for one chunk delay. */
function wait() {
  return new Promise((resolve) => setTimeout(resolve, CHUNK_DELAY_MS));
}

/** Answer one chat completion request with a streamed answer. */
async function answerCompletion(request, response) {
  let body = '';
  request.on('data', (part) => {
    body += part;
  });
  request.on('end', async () => {
    const answer = readAnswer(readBody(body), readPrompt(body));
    response.writeHead(200, {
      'content-type': 'text/event-stream',
      'cache-control': 'no-cache',
      connection: 'keep-alive',
    });
    for (const part of buildChunks(answer)) {
      response.write(part);
      await wait();
    }
    response.end();
  });
}

const server = createServer((request, response) => {
  const path = request.url ?? '';

  if (request.method === 'GET' && path.startsWith('/health')) {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end('{"status":"ok"}');
    return;
  }

  // The settings page reads the model list of the server, so the daemon
  // can offer a dropdown instead of a free text field.
  if (request.method === 'GET' && path.startsWith('/v1/models')) {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end(
      JSON.stringify({
        object: 'list',
        data: [
          { id: 'fake-4b', object: 'model' },
          { id: 'fake-small', object: 'model' },
        ],
      }),
    );
    return;
  }

  if (request.method === 'POST' && path.startsWith('/v1/chat/completions')) {
    void answerCompletion(request, response);
    return;
  }

  response.writeHead(404, { 'content-type': 'application/json' });
  response.end('{"error":"not found"}');
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`fake llama server listens on http://127.0.0.1:${PORT}`);
});
