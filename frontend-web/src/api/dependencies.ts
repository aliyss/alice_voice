/**
 * The dependency endpoint of the daemon.
 *
 * A setting belongs to a place that stores it and to a service that makes
 * it work. The settings page reads which of those answer here, so it can
 * disable a control that cannot take effect and say why.
 */
import type { ApiResult } from '~/types/bridge';
import type { DependenciesDto } from '~/types/dto';

import { server$ } from '@builder.io/qwik-city';

import { BackendError, backendGet } from '~/lib/backend-client';

/** The path of the dependency endpoint. */
const DEPENDENCIES_PATH = '/api/v1/dependencies';

/** The reply of the dependency function. */
export type DependenciesResult = ApiResult<DependenciesDto>;

/** Read which dependencies of the settings surface answer. */
export const getDependencies = server$(
  async (): Promise<DependenciesResult> => {
    try {
      const dependencies = await backendGet<DependenciesDto>(DEPENDENCIES_PATH);
      return { failed: false, data: dependencies };
    } catch (error) {
      if (error instanceof BackendError) {
        return { failed: true, message: error.message };
      }
      return { failed: true, message: 'The daemon did not answer.' };
    }
  },
);
