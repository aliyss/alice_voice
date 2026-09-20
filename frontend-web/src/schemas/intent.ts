/**
 * The validation schema of an intent.
 *
 * A `server$` function parses the input with this schema before it calls
 * the backend REST API. The backend checks the same rules again, so the
 * two never drift apart on the shape of a stored intent.
 */
import * as v from 'valibot';

/** The longest name of an entity. */
const MAX_ENTITY_NAME_LEN = 40;

/** The longest script of a script entity. */
const MAX_ENTITY_SCRIPT_LEN = 2000;

/** The largest number of phrases of one intent. */
const MAX_EXAMPLE_COUNT = 12;

/** The longest phrase of one intent. */
const MAX_EXAMPLE_LEN = 80;

/** The largest number of entities of one intent. */
const MAX_ENTITY_COUNT = 8;

/** The longest name of an intent. */
const MAX_NAME_LEN = 60;

/** The longest description of an intent. */
const MAX_DESCRIPTION_LEN = 240;

/** The longest command of an intent. */
const MAX_COMMAND_LEN = 2000;

/** One entity of an intent. */
export const intentEntitySchema = v.pipe(
  v.object({
    name: v.pipe(
      v.string(),
      v.nonEmpty('Give the entity a name.'),
      v.maxLength(MAX_ENTITY_NAME_LEN, 'An entity name is too long.'),
    ),
    kind: v.picklist(
      ['open', 'closed', 'script'],
      'Choose open, closed, or script.',
    ),
    values: v.array(v.pipe(v.string(), v.nonEmpty('Remove the empty value.'))),
    script: v.optional(
      v.pipe(
        v.string(),
        v.maxLength(MAX_ENTITY_SCRIPT_LEN, 'The script is too long.'),
      ),
    ),
    required: v.optional(v.boolean(), true),
  }),
  v.check(
    (entity) => entity.kind !== 'closed' || entity.values.length > 0,
    'A closed entity needs at least one value.',
  ),
  v.check(
    (entity) =>
      entity.kind !== 'script' || (entity.script ?? '').trim().length > 0,
    'A script entity needs a script.',
  ),
);

/** One intent of the daemon. */
export const intentSchema = v.object({
  name: v.pipe(
    v.string(),
    v.nonEmpty('Give the intent a name.'),
    v.maxLength(MAX_NAME_LEN, 'The name is too long.'),
  ),
  description: v.pipe(
    v.string(),
    v.maxLength(MAX_DESCRIPTION_LEN, 'The description is too long.'),
  ),
  command: v.pipe(
    v.string(),
    v.nonEmpty('Give the intent a command.'),
    v.maxLength(MAX_COMMAND_LEN, 'The command is too long.'),
  ),
  entities: v.pipe(
    v.array(intentEntitySchema),
    v.maxLength(MAX_ENTITY_COUNT, 'An intent takes at most eight entities.'),
  ),
  examples: v.optional(
    v.pipe(
      v.array(
        v.pipe(
          v.string(),
          v.nonEmpty('Remove the empty phrase.'),
          v.maxLength(MAX_EXAMPLE_LEN, 'A phrase is too long.'),
        ),
      ),
      v.maxLength(MAX_EXAMPLE_COUNT, 'An intent takes at most twelve phrases.'),
    ),
    [],
  ),
});

/** The input of one entity of an intent. */
export type IntentEntityInput = v.InferInput<typeof intentEntitySchema>;

/** The input of the intent form. */
export type IntentInput = v.InferInput<typeof intentSchema>;
