/**
 * `InputsSection` shows every surface and state of `TextInput` and
 * `Select`.
 *
 * The section prints the value it holds, so the binding between the field
 * and the caller is easy to check.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { Select } from '~/components/ui/select';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

export const InputsSection = component$(() => {
  const value = useSignal('');
  const locked = useSignal(false);
  const choice = useSignal('hybrid');

  const handleInput$ = $((event: Event) => {
    value.value = (event.target as HTMLInputElement).value;
  });

  const handleChoice$ = $((event: Event) => {
    choice.value = (event.target as HTMLSelectElement).value;
  });

  const handleToggle$ = $(() => {
    locked.value = !locked.value;
  });

  return (
    <Card label="TextInput">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Field surface"
          note="the live value is printed under the field"
        >
          <Stack gap="md" class="w-full max-w-sm">
            <TextInput
              name="sample"
              ariaLabel="Sample value"
              surface="field"
              placeholder="Type a value"
              value={value.value}
              disabled={locked.value}
              onInput$={handleInput$}
            />
            <TextInput
              name="locked"
              ariaLabel="Read only value"
              surface="field"
              value="The daemon owns this value"
              disabled
            />
            <Text size="micro" tone="faint">
              {`value "${value.value}" length ${value.value.length}`}
            </Text>
            <Button size="sm" variant="outline" onClick$={handleToggle$}>
              {locked.value ? 'Enable the field' : 'Disable the field'}
            </Button>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Select"
          note="one value out of a known list, enabled and disabled"
        >
          <Stack gap="md" class="w-full max-w-sm">
            <Select
              name="sample-select"
              ariaLabel="Sample choice"
              value={choice.value}
              options={[
                { value: 'llama', label: 'llama.cpp' },
                { value: 'gliner', label: 'GLiNER' },
                { value: 'hybrid', label: 'Both' },
              ]}
              onChange$={handleChoice$}
            />
            <Select
              name="locked-select"
              ariaLabel="Read only choice"
              value="hybrid"
              options={[{ value: 'hybrid', label: 'Both' }]}
              disabled
            />
            <Text size="micro" tone="faint">
              {`chosen "${choice.value}"`}
            </Text>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Plain surface and textarea"
          note="a plain field takes the box of the parent"
        >
          <Stack gap="md" class="w-full max-w-sm">
            <TextInput
              name="plain"
              ariaLabel="Plain value"
              placeholder="No box of its own"
              value=""
            />
            <TextInput
              kind="textarea"
              name="notes"
              ariaLabel="Sample note"
              surface="field"
              placeholder="A textarea grows with the content"
              value=""
              class="max-h-40"
            />
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
