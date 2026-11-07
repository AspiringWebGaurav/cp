export interface SystemInfo {
  computerName: string;
  status: string;
  hardware: string;
  engineVersion: string;
  transfersCompleted: number;
  volumeMoved: string;
  speedRating: string;
  processor: string;
  memory: string;
}

export interface ClipboardFileInfo {
  filePaths: string[];
  fileCount: number;
  primaryName: string;
  totalBytes: number;
  formattedSize: string;
  activeExplorerPath: string | null;
}

export interface ProgressUpdate {
  bytesCopied: number;
  totalBytes: number;
  formattedCopied: string;
  formattedTotal: string;
  percentage: number;
  speed: string;
  eta: string;
  isFinished: boolean;
}

export interface TransferState {
  isActive: boolean;
  isPaused: boolean;
  source: string;
  destination: string;
  currentFile: string;
  progress: number;
  speed: string;
  bytesTransferred: string;
  totalBytes: string;
  eta: string;
}

export interface ReleaseManifest {
  version: string;
  notes?: string;
  pub_date?: string;
  url?: string;
  size?: number;
  signature?: string;
  platforms?: Record<string, { url?: string; signature?: string }>;
}

export interface UpdateDownloadProgress {
  downloaded_bytes: number;
  total_bytes: number;
  percentage: number;
  speed: string;
  stage: 'downloading' | 'verifying' | 'staging' | 'ready';
}


