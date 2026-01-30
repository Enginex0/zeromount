export function getLuminance(hex: string): number {
  const rgb = hex.replace('#', '').match(/.{2}/g)!.map(x => parseInt(x, 16) / 255);
  const [r, g, b] = rgb.map(c => c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function getContrastText(bgHex: string): string {
  return getLuminance(bgHex) > 0.5 ? '#1A1A2E' : '#FFFFFF';
}

export const accentPresets: Record<string, { gradient: string; textAccent: string; rgb: string }> = {
  '#00BCD4': { // Cyan
    gradient: 'linear-gradient(135deg, #00BCD4 0%, #00E5FF 50%, #18FFFF 100%)',
    textAccent: '#00E5FF',
    rgb: '0, 188, 212',
  },
  '#FF8E53': { // Orange
    gradient: 'linear-gradient(135deg, #FF8E53 0%, #FFA040 50%, #FFD93D 100%)',
    textAccent: '#FF8E53',
    rgb: '255, 142, 83',
  },
  '#FFC107': { // Gold
    gradient: 'linear-gradient(135deg, #FFC107 0%, #FFD54F 50%, #FFEB3B 100%)',
    textAccent: '#FFC107',
    rgb: '255, 193, 7',
  },
  '#00D68F': { // Emerald
    gradient: 'linear-gradient(135deg, #00D68F 0%, #00E5A0 50%, #69F0AE 100%)',
    textAccent: '#00D68F',
    rgb: '0, 214, 143',
  },
  '#00B4D8': { // Azure
    gradient: 'linear-gradient(135deg, #00B4D8 0%, #48CAE4 50%, #90E0EF 100%)',
    textAccent: '#00B4D8',
    rgb: '0, 180, 216',
  },
  '#764BA2': { // Purple
    gradient: 'linear-gradient(135deg, #764BA2 0%, #9B59B6 50%, #C084FC 100%)',
    textAccent: '#9B59B6',
    rgb: '118, 75, 162',
  },
  '#FFFFFF': { // White/Silver
    gradient: 'linear-gradient(135deg, #E0E0E0 0%, #BDBDBD 50%, #9E9E9E 100%)',
    textAccent: '#BDBDBD',
    rgb: '189, 189, 189',
  },
  '#64748B': { // Slate
    gradient: 'linear-gradient(135deg, #64748B 0%, #94A3B8 50%, #CBD5E1 100%)',
    textAccent: '#94A3B8',
    rgb: '100, 116, 139',
  },
};

export function needsDarkText(accentColor: string): boolean {
  const preset = accentPresets[accentColor];
  if (!preset) return false;
  const [r, g, b] = preset.rgb.split(',').map(n => parseInt(n.trim()));
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255 > 0.6;
}

export function getAccentStyles(accentColor: string) {
  return accentPresets[accentColor] || accentPresets['#00BCD4'];
}

export const darkTheme = {
  gradientPrimary: 'linear-gradient(135deg, #FF6B6B 0%, #FF8E53 50%, #FFC107 100%)',
  gradientSecondary: 'linear-gradient(180deg, #1A1A2E 0%, #0F0F1A 100%)',

  colorSuccess: '#00D68F',
  colorSuccessGlow: 'rgba(0, 214, 143, 0.4)',
  colorWarning: '#FFB800',
  colorWarningGlow: 'rgba(255, 184, 0, 0.4)',
  colorError: '#FF3D71',
  colorErrorGlow: 'rgba(255, 61, 113, 0.4)',
  colorInfo: '#00B4D8',
  colorInfoGlow: 'rgba(0, 180, 216, 0.4)',

  bgPrimary: 'linear-gradient(180deg, #0F0F1A 0%, #1A1A2E 100%)',
  bgSurface: 'rgba(255, 255, 255, 0.05)',
  bgSurfaceElevated: 'rgba(255, 255, 255, 0.08)',
  bgSurfaceHover: 'rgba(255, 255, 255, 0.12)',

  glassBg: 'rgba(255, 255, 255, 0.06)',
  glassBorder: 'rgba(255, 255, 255, 0.1)',
  glassBlur: 'blur(20px)',

  textPrimary: '#FFFFFF',
  textSecondary: 'rgba(255, 255, 255, 0.7)',
  textTertiary: 'rgba(255, 255, 255, 0.5)',
  textAccent: '#FF8E53',

  fontDisplay: "'Space Grotesk', system-ui, sans-serif",
  fontBody: "'Inter', system-ui, sans-serif",
  fontMono: "'JetBrains Mono', 'Fira Code', monospace",

  radiusSmall: '8px',
  radiusMedium: '12px',
  radiusLarge: '16px',
  radiusXLarge: '24px',

  shadowSmall: '0 2px 8px rgba(0, 0, 0, 0.2)',
  shadowMedium: '0 8px 24px rgba(0, 0, 0, 0.3)',
  shadowLarge: '0 20px 40px rgba(0, 0, 0, 0.4)',
  shadowGlow: '0 0 20px rgba(255, 107, 107, 0.3)',

  accentRgb: '255, 107, 107',
};

export const lightTheme: typeof darkTheme = {
  ...darkTheme,
  gradientSecondary: 'linear-gradient(180deg, #FFFFFF 0%, #F5F5F5 100%)',

  bgPrimary: 'linear-gradient(180deg, #F5F5F5 0%, #FFFFFF 100%)',
  bgSurface: 'rgba(0, 0, 0, 0.03)',
  bgSurfaceElevated: 'rgba(0, 0, 0, 0.05)',
  bgSurfaceHover: 'rgba(0, 0, 0, 0.08)',

  glassBg: 'rgba(0, 0, 0, 0.03)',
  glassBorder: 'rgba(0, 0, 0, 0.1)',

  textPrimary: '#1A1A2E',
  textSecondary: 'rgba(0, 0, 0, 0.7)',
  textTertiary: 'rgba(0, 0, 0, 0.5)',
  textAccent: '#E65100',

  shadowSmall: '0 2px 8px rgba(0, 0, 0, 0.1)',
  shadowMedium: '0 8px 24px rgba(0, 0, 0, 0.15)',
  shadowLarge: '0 20px 40px rgba(0, 0, 0, 0.2)',
  shadowGlow: '0 0 20px rgba(255, 107, 107, 0.4)',
};

export const amoledTheme: typeof darkTheme = {
  ...darkTheme,
  gradientSecondary: 'linear-gradient(180deg, #000000 0%, #050505 100%)',

  bgPrimary: '#000000',
  bgSurface: 'rgba(255, 255, 255, 0.03)',
  bgSurfaceElevated: 'rgba(255, 255, 255, 0.05)',
  bgSurfaceHover: 'rgba(255, 255, 255, 0.08)',

  glassBg: 'rgba(255, 255, 255, 0.02)',
  glassBorder: 'rgba(255, 255, 255, 0.08)',

  shadowSmall: '0 2px 8px rgba(0, 0, 0, 0.5)',
  shadowMedium: '0 8px 24px rgba(0, 0, 0, 0.6)',
  shadowLarge: '0 20px 40px rgba(0, 0, 0, 0.7)',
};

export const theme = darkTheme;

export function applyTheme(themeObj: typeof darkTheme, accentColor?: string) {
  const root = document.documentElement;
  root.style.setProperty('--bg-primary', themeObj.bgPrimary);
  root.style.setProperty('--bg-surface', themeObj.bgSurface);
  root.style.setProperty('--bg-surface-elevated', themeObj.bgSurfaceElevated);
  root.style.setProperty('--bg-surface-hover', themeObj.bgSurfaceHover);
  root.style.setProperty('--glass-bg', themeObj.glassBg);
  root.style.setProperty('--glass-border', themeObj.glassBorder);
  root.style.setProperty('--text-primary', themeObj.textPrimary);
  root.style.setProperty('--text-secondary', themeObj.textSecondary);
  root.style.setProperty('--text-tertiary', themeObj.textTertiary);
  root.style.setProperty('--gradient-secondary', themeObj.gradientSecondary);
  root.style.setProperty('--color-success', themeObj.colorSuccess);
  root.style.setProperty('--color-success-glow', themeObj.colorSuccessGlow);
  root.style.setProperty('--color-warning', themeObj.colorWarning);
  root.style.setProperty('--color-warning-glow', themeObj.colorWarningGlow);
  root.style.setProperty('--color-error', themeObj.colorError);
  root.style.setProperty('--color-error-glow', themeObj.colorErrorGlow);
  root.style.setProperty('--color-info', themeObj.colorInfo);
  root.style.setProperty('--color-info-glow', themeObj.colorInfoGlow);
  root.style.setProperty('--shadow-small', themeObj.shadowSmall);
  root.style.setProperty('--shadow-medium', themeObj.shadowMedium);
  root.style.setProperty('--shadow-large', themeObj.shadowLarge);

  // Apply accent-based styles
  const accentStyles = getAccentStyles(accentColor || '#FF6B6B');
  root.style.setProperty('--gradient-primary', accentStyles.gradient);
  root.style.setProperty('--text-accent', accentStyles.textAccent);
  root.style.setProperty('--accent-rgb', accentStyles.rgb);
  root.style.setProperty('--shadow-glow', `0 0 20px rgba(${accentStyles.rgb}, 0.3)`);
}

// Apply only accent changes (for when just the accent color changes)
export function applyAccent(accentColor: string) {
  const root = document.documentElement;
  const accentStyles = getAccentStyles(accentColor);
  root.style.setProperty('--gradient-primary', accentStyles.gradient);
  root.style.setProperty('--text-accent', accentStyles.textAccent);
  root.style.setProperty('--accent-rgb', accentStyles.rgb);
  root.style.setProperty('--shadow-glow', `0 0 20px rgba(${accentStyles.rgb}, 0.3)`);
}

export const springConfigs = {
  snappy: { mass: 1, stiffness: 300, damping: 20 },
  bouncy: { mass: 1, stiffness: 200, damping: 10 },
  smooth: { mass: 1, stiffness: 100, damping: 20 },
  elastic: { mass: 1, stiffness: 400, damping: 8 },
};
