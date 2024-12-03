export interface SharedConfig {
  singleton?: boolean;
  strictVersion?: boolean;
  requiredVersion?: string;
  version?: string;
  eager?: boolean;
}

export interface FederationConfig {
  name: string;
  filename?: string;
  exposes?: Record<string, string>;
  shared?: Record<string, SharedConfig>;
  remotes?: Record<string, string>;
}
