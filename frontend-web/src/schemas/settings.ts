/**
 * The validation schema of the settings input.
 *
 * A `server$` function parses the input with this schema before it calls
 * the backend REST API.
 */
import * as v from 'valibot';

/** One toggle for the queue feature. */
export const queueSettingsSchema = v.object({
  queueEnabled: v.boolean(),
});

/** The engines that can read the intent of a message. */
export const resolverBackendsSchema = v.picklist([
  'router',
  'hybrid',
  'llama',
  'gliner',
] as const);

/** The devices a built in model can run on. */
export const glinerDevicesSchema = v.picklist(['auto', 'cpu', 'cuda'] as const);

/** Where the router reads the vectors of the catalog. */
export const embedSourcesSchema = v.picklist(['server', 'local'] as const);

/** The evidence the retrieval stage of the router ranks the catalog with. */
export const retrieveEnginesSchema = v.picklist([
  'lexical',
  'dense',
  'hybrid',
] as const);

/** How the decision stage of the router chooses one of the short list. */
export const decideEnginesSchema = v.picklist([
  'score',
  'rerank',
  'generative',
] as const);

/** How the extraction stage of the router reads the entity values. */
export const extractEnginesSchema = v.picklist([
  'lists',
  'spans',
  'generative',
] as const);

/** How a mention is read against the values of an entity. */
export const listMatchesSchema = v.picklist([
  'lexical',
  'dense',
  'both',
] as const);

/** One probability between zero and one. */
const probabilitySchema = (message: string) =>
  v.pipe(
    v.number(message),
    v.minValue(0, 'The value is 0 or more.'),
    v.maxValue(1, 'The value is 1 or less.'),
  );

/** One weight of zero or more. */
const weightSchema = (message: string) =>
  v.pipe(v.number(message), v.minValue(0, 'The weight is 0 or more.'));

/** The stages of the layered router. */
const routerSchema = {
  routerFastPath: v.boolean(),
  routerRetrieve: retrieveEnginesSchema,
  routerDecide: decideEnginesSchema,
  routerExtract: extractEnginesSchema,
  routerTopK: v.pipe(
    v.number('Enter the size of the short list.'),
    v.minValue(1, 'The short list holds at least one intent.'),
    v.maxValue(64, 'The short list holds at most 64 intents.'),
  ),
  routerFloor: probabilitySchema(
    'Enter the smallest score the decision accepts.',
  ),
  routerMargin: probabilitySchema(
    'Enter the smallest distance between the best and the second best.',
  ),
  routerLexicalWeight: weightSchema('Enter the weight of the words.'),
  routerDenseWeight: weightSchema('Enter the weight of the embeddings.'),
  routerEmbedModel: v.pipe(
    v.string(),
    v.nonEmpty('Enter the model name the embedding server answers to.'),
  ),
  routerModelsDir: v.pipe(
    v.string(),
    v.nonEmpty('Enter the directory that holds the built in models.'),
  ),
  routerEmbedSource: embedSourcesSchema,
  routerEmbedLocalModel: v.pipe(
    v.string(),
    v.nonEmpty('Choose a built in embedding model.'),
  ),
  routerRerankModel: v.pipe(
    v.string(),
    v.nonEmpty('Choose a built in reranker.'),
  ),
  routerLocalDevice: glinerDevicesSchema,
  routerPhraseGate: v.boolean(),
  routerListMatch: listMatchesSchema,
  routerListFloor: probabilitySchema(
    'Enter the smallest similarity a value needs.',
  ),
};

/** The address and the model of the intent resolver. */
export const resolverSettingsSchema = v.object({
  resolverBackend: resolverBackendsSchema,
  resolverBaseUrl: v.pipe(
    v.string(),
    v.nonEmpty('Enter the address of the llama.cpp server.'),
    v.url('Enter an address that starts with http://.'),
  ),
  resolverModel: v.pipe(
    v.string(),
    v.nonEmpty('Enter the model name the server answers to.'),
  ),
  glinerModel: v.pipe(
    v.string(),
    v.nonEmpty('Choose a built in GLiNER model.'),
  ),
  glinerDevice: glinerDevicesSchema,
  glinerThreshold: v.pipe(
    v.number('Enter the smallest probability a label needs to count.'),
    v.minValue(0, 'The threshold is 0 or more.'),
    v.maxValue(1, 'The threshold is 1 or less.'),
  ),
  ...routerSchema,
});

/**
 * The settings of the daemon.
 *
 * Every field is optional, so a caller changes one setting without
 * sending the others. The daemon keeps a value it does not receive.
 */
export const settingsSchema = v.object({
  queueEnabled: v.optional(v.boolean()),
  resolverBackend: v.optional(resolverBackendsSchema),
  resolverBaseUrl: v.optional(
    v.pipe(
      v.string(),
      v.nonEmpty('Enter the address of the llama.cpp server.'),
      v.url('Enter an address that starts with http://.'),
    ),
  ),
  resolverModel: v.optional(
    v.pipe(
      v.string(),
      v.nonEmpty('Enter the model name the server answers to.'),
    ),
  ),
  glinerModel: v.optional(
    v.pipe(v.string(), v.nonEmpty('Choose a built in GLiNER model.')),
  ),
  glinerDevice: v.optional(glinerDevicesSchema),
  glinerThreshold: v.optional(
    v.pipe(
      v.number('Enter the smallest probability a label needs to count.'),
      v.minValue(0, 'The threshold is 0 or more.'),
      v.maxValue(1, 'The threshold is 1 or less.'),
    ),
  ),
  routerFastPath: v.optional(v.boolean()),
  routerRetrieve: v.optional(retrieveEnginesSchema),
  routerDecide: v.optional(decideEnginesSchema),
  routerExtract: v.optional(extractEnginesSchema),
  routerTopK: v.optional(
    v.pipe(
      v.number('Enter the size of the short list.'),
      v.minValue(1, 'The short list holds at least one intent.'),
      v.maxValue(64, 'The short list holds at most 64 intents.'),
    ),
  ),
  routerFloor: v.optional(
    probabilitySchema('Enter the smallest score the decision accepts.'),
  ),
  routerMargin: v.optional(
    probabilitySchema(
      'Enter the smallest distance between the best and the second best.',
    ),
  ),
  routerLexicalWeight: v.optional(
    weightSchema('Enter the weight of the words.'),
  ),
  routerDenseWeight: v.optional(
    weightSchema('Enter the weight of the embeddings.'),
  ),
  routerEmbedModel: v.optional(
    v.pipe(
      v.string(),
      v.nonEmpty('Enter the model name the embedding server answers to.'),
    ),
  ),
  routerModelsDir: v.optional(
    v.pipe(
      v.string(),
      v.nonEmpty('Enter the directory that holds the built in models.'),
    ),
  ),
  routerEmbedSource: v.optional(embedSourcesSchema),
  routerEmbedLocalModel: v.optional(
    v.pipe(v.string(), v.nonEmpty('Choose a built in embedding model.')),
  ),
  routerRerankModel: v.optional(
    v.pipe(v.string(), v.nonEmpty('Choose a built in reranker.')),
  ),
  routerLocalDevice: v.optional(glinerDevicesSchema),
  routerPhraseGate: v.optional(v.boolean()),
  routerListMatch: v.optional(listMatchesSchema),
  routerListFloor: v.optional(
    probabilitySchema('Enter the smallest similarity a value needs.'),
  ),
  /**
   * The sentences the user tries against the resolver. They are a list the
   * page writes on its own, so they never travel with the resolver form.
   */
  previewSentences: v.optional(
    v.pipe(
      v.array(v.string()),
      v.maxLength(32, 'Keep the list at 32 sentences or fewer.'),
    ),
  ),
});

/** The input of the queue toggle. */
export type QueueSettingsInput = v.InferInput<typeof queueSettingsSchema>;

/** The input of the resolver settings form. */
export type ResolverSettingsInput = v.InferInput<typeof resolverSettingsSchema>;

/** The input of the update settings function. */
export type SettingsInput = v.InferInput<typeof settingsSchema>;
