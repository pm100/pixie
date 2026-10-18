export type ViewerMode = 'window' | 'popout';

/** A stored preference may be anything; only the two real modes count. */
export const parseViewerMode = (v: unknown): ViewerMode => (v === 'popout' ? 'popout' : 'window');
