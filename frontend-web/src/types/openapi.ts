/**
 * Minimal OpenAPI types for the frontend docs.
 *
 * The backend owns the spec at `backend/config/openapi.yaml` and serves the
 * live spec at `GET /api/docs/openapi.json`. The frontend copies this file
 * for no purpose: the docs page reads the live spec and the DTOs already
 * live in `src/types/dto.ts`.
 */

export interface OpenApiInfoDto {
  title: string;
  version: string;
  description?: string;
}

export interface OpenApiServerDto {
  url: string;
  description?: string;
}

export interface OpenApiSchemaDto {
  type?: string | string[];
  format?: string;
  description?: string;
  enum?: string[];
  example?: string;
  properties?: Record<string, OpenApiSchemaDto>;
  required?: string[];
  items?: OpenApiSchemaDto;
  allOf?: OpenApiSchemaDto[];
  oneOf?: OpenApiSchemaDto[];
  anyOf?: OpenApiSchemaDto[];
  $ref?: string;
  nullable?: boolean;
}

export interface OpenApiParameterDto {
  name: string;
  in: 'query' | 'path' | 'header' | 'cookie';
  description?: string;
  required?: boolean;
  schema?: OpenApiSchemaDto;
}

export interface OpenApiRequestBodyDto {
  description?: string;
  required?: boolean;
  content?: Record<string, { schema?: OpenApiSchemaDto }>;
}

export interface OpenApiResponseDto {
  description?: string;
  content?: Record<string, { schema?: OpenApiSchemaDto }>;
}

export interface OpenApiOperationDto {
  tags?: string[];
  summary?: string;
  description?: string;
  operationId?: string;
  parameters?: OpenApiParameterDto[];
  requestBody?: OpenApiRequestBodyDto;
  responses?: Record<string, OpenApiResponseDto>;
  deprecated?: boolean;
}

export type OpenApiPathItemDto = Record<string, OpenApiOperationDto>;

export interface OpenApiDto {
  openapi: string;
  info: OpenApiInfoDto;
  servers?: OpenApiServerDto[];
  paths: Record<string, OpenApiPathItemDto>;
  components?: {
    schemas?: Record<string, OpenApiSchemaDto>;
  };
}
