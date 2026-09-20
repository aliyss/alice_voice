/**
 * `ServerFields` reads the model server of the resolver.
 *
 * A stage of the router reads the server only in one of two ways: the
 * retrieval stage asks it for embeddings, and a generative decision or
 * extraction stage asks it for a chat answer. The fields of the server
 * therefore sit in the stage that reads it, and the address is one database
 * setting, so every stage that shows it writes the same value.
 *
 * A stage that reads embeddings alone hides the chat model (`showModel`),
 * because that stage never sends a chat request.
 */
import type { QRL } from '@builder.io/qwik';

import type { SelectOption } from '~/components/ui/select';

import type { LabelBudgetDto } from '~/types/dto';

import { $, component$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { FieldLabel } from '~/components/ui/field-label';
import { Select } from '~/components/ui/select';
import { Stack } from '~/components/ui/stack';
import { TextInput } from '~/components/ui/text-input';

/** The props of `ServerFields`. */
export interface ServerFieldsProps {
  /** The address of the server. */
  baseUrl: string;
  /** The chat model the server answers to. */
  model: string;
  /** The models the dropdown offers. */
  modelOptions: SelectOption[];
  /** The address the server answers at, or null while it does not answer. */
  resolvedBaseUrl: string | null;
  /** True when the value can be stored. */
  canStore: boolean;
  /** True when the server answers. */
  llamaReachable: boolean;
  /** True while a save request is in flight. */
  pending: boolean;
  /** Draw the chat model. It is on by default. */
  showModel?: boolean;
  /**
   * The label budget of the catalog, or null. A model that names one intent
   * per letter reads fewer intents than the catalog holds, so the warning
   * belongs to the configuration of the server that answers.
   */
  budget?: LabelBudgetDto | null;
  /** Report a new address. */
  onBaseUrl$: QRL<(value: string) => void>;
  /** Report a new chat model. */
  onModel$: QRL<(value: string) => void>;
}

export const ServerFields = component$<ServerFieldsProps>((props) => {
  return (
    <Stack gap="md">
      <Stack gap="xs">
        <FieldLabel
          label="Server address"
          hint="The address is a database setting, so it stays editable while the server is down. The model below follows the server."
        />
        <TextInput
          kind="input"
          surface="field"
          name="resolverBaseUrl"
          ariaLabel="Address of the llama.cpp server"
          placeholder="http://127.0.0.1:8012/v1"
          value={props.baseUrl}
          disabled={props.pending || !props.canStore}
          onInput$={$((event: Event) => {
            props.onBaseUrl$((event.target as HTMLInputElement).value);
          })}
        />
      </Stack>

      {props.showModel === false ? null : (
        <Stack gap="xs">
          <FieldLabel
            label="Model"
            hint={
              props.canStore && props.llamaReachable && props.resolvedBaseUrl
                ? `The list comes from ${props.resolvedBaseUrl}/models, so the name is one the server really answers to.`
                : 'The model the server names the intent with. It has to answer to the name exactly.'
            }
          />
          {props.canStore &&
          props.llamaReachable &&
          props.modelOptions.length > 0 ? (
            <Select
              name="resolverModel"
              ariaLabel="Model of the llama.cpp server"
              value={props.model}
              options={props.modelOptions}
              disabled={props.pending}
              onChange$={$((event: Event) => {
                props.onModel$((event.target as HTMLSelectElement).value);
              })}
            />
          ) : (
            <TextInput
              kind="input"
              surface="field"
              name="resolverModel"
              ariaLabel="Model name of the llama.cpp server"
              placeholder="qwen3.5-4b"
              value={props.model}
              disabled
              onInput$={$(() => {
                /* The field is disabled until its server answers. */
              })}
            />
          )}
          {props.canStore && !props.llamaReachable ? (
            <Alert
              tone="warn"
              title="The server does not answer"
              message="The field shows the stored name. Start the server and reload to pick from the models it offers."
            />
          ) : null}
        </Stack>
      )}

      {/* A server that names one intent per letter reads fewer intents than
          the catalog holds, so the limit belongs to the server that names
          them rather than to the models that read the labels. A stage that
          only asks for embeddings names no intent at all. */}
      {props.showModel !== false &&
      props.budget &&
      props.budget.intentLimit !== null &&
      props.budget.intents > props.budget.intentLimit ? (
        <Alert
          tone="warn"
          title="Many intents"
          message={`This reader names one letter per intent, so it offers the first ${props.budget.intentLimit}. Remove intents, or read the intent with a reader that ranks the whole catalog.`}
        />
      ) : null}
    </Stack>
  );
});
