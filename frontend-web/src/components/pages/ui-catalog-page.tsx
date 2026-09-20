/**
 * `UiCatalogPage` lists every primitive of the design system.
 *
 * Each section shows one primitive with its individual states. The page
 * reads no data, so a route that renders it needs no loader and the page
 * takes no props.
 *
 * The header of the page is also the live sample of `PageHeader`, and the
 * status strip above it is the sample of the shell navigation.
 */
import { component$ } from '@builder.io/qwik';

import { AuraSection } from '~/components/sections/ui/aura-section';
import { BadgesSection } from '~/components/sections/ui/badges-section';
import { ButtonsSection } from '~/components/sections/ui/buttons-section';
import { DataSection } from '~/components/sections/ui/data-section';
import { FeedbackSection } from '~/components/sections/ui/feedback-section';
import { FlowSection } from '~/components/sections/ui/flow-section';
import { InputsSection } from '~/components/sections/ui/inputs-section';
import { NavSection } from '~/components/sections/ui/nav-section';
import { PanelSection } from '~/components/sections/ui/panel-section';
import { SurfacesSection } from '~/components/sections/ui/surfaces-section';
import { SwitcherSection } from '~/components/sections/ui/switcher-section';
import { TabsSection } from '~/components/sections/ui/tabs-section';
import { TextSection } from '~/components/sections/ui/text-section';
import { TokensSection } from '~/components/sections/ui/tokens-section';
import { PageHeader } from '~/components/ui/page-header';
import { ScrollArea } from '~/components/ui/scroll-area';
import { Stack } from '~/components/ui/stack';

export const UiCatalogPage = component$(() => {
  return (
    <ScrollArea ariaLabel="Design system catalog" class="flex-1">
      <Stack gap="xl" class="mx-auto w-full max-w-5xl px-8 py-6">
        <PageHeader
          title="Design system"
          description="Every primitive of the web frontend with its individual states."
          meta="frontend-web"
        />
        <TextSection />
        <ButtonsSection />
        <BadgesSection />
        <InputsSection />
        <DataSection />
        <FeedbackSection />
        <SurfacesSection />
        <NavSection />
        <TabsSection />
        <SwitcherSection />
        <PanelSection />
        <FlowSection />
        <AuraSection />
        <TokensSection />
      </Stack>
    </ScrollArea>
  );
});
