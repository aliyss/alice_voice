/**
 * `FlowSection` shows the node graph primitive.
 *
 * `FlowGraph` draws blocks on a grid and joins them with links: a main link
 * runs down the middle of a column, and a branch link turns inside the
 * gutter in front of a column. The blocks are buttons and report the one a
 * reader picks, so a caller shows the settings of a block beside the graph.
 *
 * A caller that read the route of one message hands it the blocks, the
 * links, and the tone of each: the route keeps its color, a stage that
 * fell back reads amber, a stage that refused reads red, and every other
 * block of the graph steps back.
 */
import { component$ } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Card } from '~/components/ui/card';
import { FlowGraph } from '~/components/ui/flow';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

export const FlowSection = component$(() => {
  return (
    <Card label="FlowGraph">
      <Stack gap="lg">
        <ShowcaseBlock
          title="FlowGraph, stages and places"
          note="one block per step, a branch block for every place a step reads"
        >
          <Stack gap="sm" class="w-full">
            <FlowGraph
              id="catalog-flow"
              ariaLabel="Sample flow"
              nodes={[
                {
                  id: 'server',
                  title: 'Model server',
                  detail: '127.0.0.1:8012/v1',
                  status: { label: 'Up', tone: 'ok' },
                  kind: 'place',
                  column: 0,
                  row: 1,
                },
                {
                  id: 'message',
                  title: 'The message',
                  detail: 'one text, with the turns before it',
                  kind: 'stage',
                  column: 1,
                  row: 0,
                  stage: 1,
                },
                {
                  id: 'retrieval',
                  title: 'Retrieval',
                  detail: 'ranks the catalog and keeps the short list',
                  status: { label: 'Words' },
                  kind: 'stage',
                  column: 1,
                  row: 1,
                  stage: 2,
                },
                {
                  id: 'decision',
                  title: 'Decision',
                  detail: 'chooses one of the short list, or none',
                  status: { label: 'Model', tone: 'ok' },
                  kind: 'stage',
                  column: 1,
                  row: 2,
                  stage: 3,
                },
                {
                  id: 'refused',
                  title: 'No intent matched',
                  detail: 'the turn refuses instead of running',
                  kind: 'exit',
                  column: 2,
                  row: 2,
                },
              ]}
              edges={[
                {
                  id: 'a',
                  from: 'message',
                  to: 'retrieval',
                  label: 'one message',
                  kind: 'main',
                },
                {
                  id: 'b',
                  from: 'retrieval',
                  to: 'decision',
                  label: 'short list of 8',
                  kind: 'main',
                },
                {
                  id: 'c',
                  from: 'server',
                  to: 'decision',
                  label: 'chat',
                  kind: 'branch',
                },
                {
                  id: 'd',
                  from: 'decision',
                  to: 'refused',
                  label: 'none',
                  kind: 'branch',
                },
              ]}
            />
            <Text size="micro" tone="faint">
              A filled block is a step of the chain, a sunken block is a place a
              step reads, and a dashed block is a turn that leaves the chain.
              The places stand in the first column, the chain in the middle, and
              the turns that leave it in the last one, so every link runs
              forward. Drag the canvas to move it, use the wheel or the controls
              to zoom, and press a block to pick it.
            </Text>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="FlowGraph, the route of one message"
          note="the route keeps its tone, every other block steps back"
        >
          <Stack gap="sm" class="w-full">
            <FlowGraph
              id="catalog-flow-route"
              ariaLabel="Sample route"
              nodes={[
                {
                  id: 'message',
                  title: 'The message',
                  detail: 'open firefox',
                  kind: 'stage',
                  column: 0,
                  row: 0,
                  stage: 1,
                },
                {
                  id: 'retrieval',
                  title: 'Retrieval',
                  detail: 'ranks the catalog and keeps the short list',
                  status: { label: 'Embeddings' },
                  kind: 'stage',
                  column: 0,
                  row: 1,
                  stage: 2,
                },
                {
                  id: 'server',
                  title: 'Model server',
                  detail: '127.0.0.1:8012/v1',
                  status: { label: 'Down', tone: 'error' },
                  kind: 'place',
                  column: 1,
                  row: 1,
                },
                {
                  id: 'decision',
                  title: 'Decision',
                  detail: 'chooses one of the short list, or none',
                  status: { label: 'Scores' },
                  kind: 'stage',
                  column: 0,
                  row: 2,
                  stage: 3,
                },
                {
                  id: 'refused',
                  title: 'No intent matched',
                  detail: 'the turn refuses instead of running',
                  kind: 'exit',
                  column: 1,
                  row: 2,
                },
              ]}
              edges={[
                {
                  id: 'a',
                  from: 'message',
                  to: 'retrieval',
                  label: 'one message',
                  kind: 'main',
                },
                {
                  id: 'b',
                  from: 'retrieval',
                  to: 'decision',
                  label: 'short list of 8',
                  kind: 'main',
                },
                {
                  id: 'c',
                  from: 'server',
                  to: 'retrieval',
                  label: 'embeddings',
                  kind: 'branch',
                },
                {
                  id: 'd',
                  from: 'decision',
                  to: 'refused',
                  label: 'none',
                  kind: 'branch',
                },
              ]}
              route={{
                // The retrieval stage fell back to the words, because the
                // server did not answer, and the decision refused.
                nodes: {
                  message: 'ok',
                  retrieval: 'warn',
                  decision: 'error',
                  refused: 'error',
                },
                edges: {
                  a: 'ok',
                  b: 'warn',
                  d: 'error',
                },
              }}
            />{' '}
            <Text size="micro" tone="faint">
              A route paints the block and the link it walked: green for a stage
              that answered, amber for a stage that fell back to another reader,
              and red for the stage that refused. Every block the route did not
              reach steps back, and the value of a link of the route steps up
              from faint to default rather than taking the color of its link,
              because a status color on a short machine value at 11px is hard to
              read.
            </Text>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
