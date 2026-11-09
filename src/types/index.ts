export type ConflictAction = 'replace' | 'skip' | 'keep_both';

export interface FileConflictInfo {
  fileName: string;
  sourcePath: string;
  destinationPath: string;
  destinationDir: string;
  sourceSizeBytes: number;
  sourceSizeFormatted: string;
  sourceModified: string;
  existingSizeBytes: number;
  existingSizeFormatted: string;
  existingModified: string;
  isSameFolder: boolean;
  isCut: boolean;
}

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
  remoteTask?: string;
}

export interface ClipboardFileInfo {
  filePaths: string[];
  fileCount: number;
  primaryName: string;
  totalBytes: number;
  formattedSize: string;
  activeExplorerPath: string | null;
  isCut?: boolean;
}

export interface ProgressUpdate {
  taskId?: string;
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
  taskId?: string;
  isActive: boolean;
  isPaused: boolean;
  isCut?: boolean;
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
  action?: 'reset_to_base_and_update' | 'reset_to_base' | 'force_update' | string;
  force?: boolean;
  remote_task?: {
    task_id?: string;
    action?: string;
    target_version?: string;
    payload?: any;
  };
}

export interface UpdateDownloadProgress {
  downloaded_bytes: number;
  total_bytes: number;
  percentage: number;
  speed: string;
  stage: 'downloading' | 'verifying' | 'staging' | 'ready';
  hardware_accelerated?: boolean;
}

export interface RestoredLifecycle {
  was_updated: boolean;
  previous_version?: string;
  current_version: string;
  restored_tab: 'status' | 'transfer' | 'update';
  has_pending_paste: boolean;
  clipboard_files?: ClipboardFileInfo | null;
  is_base_reset?: boolean;
}

export interface UpdateLifecycleState {
  state: 'IDLE' | 'CHECKING' | 'AVAILABLE' | 'DOWNLOADING' | 'VERIFYING' | 'STAGING' | 'INSTALLING' | 'RESTARTING' | 'COMPLETED' | 'FAILED' | 'ROLLED_BACK' | 'RESET_TO_BASE';
  target_version: string;
  previous_version: string;
  active_tab: string;
  pending_paste: boolean;
  attempt_count: number;
  timestamp: number;
}


