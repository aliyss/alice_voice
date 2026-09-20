/**
 * The public exports of the Alice Voice design system.
 *
 * A view imports its structure from this barrel. Do not import a raw
 * element when the design system already exports the primitive.
 */

export { Alert } from '~/components/ui/alert';
export { AppLayout } from '~/components/ui/app-layout';
export { Badge } from '~/components/ui/badge';
export { Box } from '~/components/ui/box';
export { Button } from '~/components/ui/button';
export { Card } from '~/components/ui/card';
export { ContentSwitcher } from '~/components/ui/content-switcher';
export { Disclosure } from '~/components/ui/disclosure';
export { FieldLabel } from '~/components/ui/field-label';
export { FlowGraph } from '~/components/ui/flow';
export { EmptyState } from '~/components/ui/empty-state';
export { InfoHint } from '~/components/ui/info-hint';
export { Link } from '~/components/ui/link';
export { MetadataRow } from '~/components/ui/metadata-row';
export { PageHeader } from '~/components/ui/page-header';
export { PanelGroup } from '~/components/ui/panel-group';
export { ScrollArea } from '~/components/ui/scroll-area';
export { SectionNav } from '~/components/ui/section-nav';
export { Select } from '~/components/ui/select';
export { SidePanel } from '~/components/ui/side-panel';
export { Spinner } from '~/components/ui/spinner';
export { Stack } from '~/components/ui/stack';
export { Tabs, tabId, tabPanelId } from '~/components/ui/tabs';
export { Text } from '~/components/ui/text';
export { TextInput } from '~/components/ui/text-input';
export { Tooltip } from '~/components/ui/tooltip';

export type { AlertProps, AlertTone } from '~/components/ui/alert';
export type { AppLayoutProps } from '~/components/ui/app-layout';
export type { BadgeProps, BadgeTone } from '~/components/ui/badge';
export type { BoxProps } from '~/components/ui/box';
export type {
  ButtonProps,
  ButtonSize,
  ButtonVariant,
} from '~/components/ui/button';
export type { CardProps, CardTone } from '~/components/ui/card';
export type {
  ContentSwitcherOption,
  ContentSwitcherProps,
  ContentSwitcherSize,
} from '~/components/ui/content-switcher';
export type { DisclosureProps } from '~/components/ui/disclosure';
export type { EmptyStateProps } from '~/components/ui/empty-state';
export type { FieldLabelProps } from '~/components/ui/field-label';
export type {
  FlowEdgeSpec,
  FlowGraphProps,
  FlowNodeKind,
  FlowNodeSpec,
} from '~/components/ui/flow';
export type {
  InfoHintAlign,
  InfoHintProps,
  InfoHintSide,
} from '~/components/ui/info-hint';
export type { LinkProps, LinkTone } from '~/components/ui/link';
export type { MetadataRowProps } from '~/components/ui/metadata-row';
export type { PageHeaderProps } from '~/components/ui/page-header';
export type { PanelGroupProps } from '~/components/ui/panel-group';
export type { ScrollAreaProps } from '~/components/ui/scroll-area';
export type {
  SectionNavItem,
  SectionNavProps,
  SectionNavStatus,
} from '~/components/ui/section-nav';
export type { SelectOption, SelectProps } from '~/components/ui/select';
export type { SidePanelProps } from '~/components/ui/side-panel';
export type { SpinnerProps, SpinnerSize } from '~/components/ui/spinner';
export type {
  StackAlign,
  StackDirection,
  StackGap,
  StackJustify,
  StackProps,
} from '~/components/ui/stack';
export type { TabItem, TabsProps } from '~/components/ui/tabs';
export type {
  TextProps,
  TextSize,
  TextTone,
  TextWeight,
} from '~/components/ui/text';
export type {
  TextInputKind,
  TextInputProps,
  TextInputSurface,
} from '~/components/ui/text-input';
export type { TooltipProps, TooltipSide } from '~/components/ui/tooltip';
