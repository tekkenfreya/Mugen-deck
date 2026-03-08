interface MugenLogoProps {
  size?: number;
}

export function MugenLogo({ size = 64 }: MugenLogoProps) {
  return (
    <svg
      width={size}
      height={size * 0.6}
      viewBox="0 0 120 72"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{ display: 'block' }}
    >
      <g>
        {/* Bold angular "M" — two valley strokes */}
        <path
          d="M10,58 L10,14 L32,38 L60,10 L88,38 L110,14 L110,58"
          stroke="#c0c0c8"
          strokeWidth="4"
          strokeLinejoin="miter"
          strokeLinecap="square"
          fill="none"
        />

        {/* Circuit trace nodes at vertices */}
        <rect x="7" y="11" width="6" height="6" fill="#c0c0c8" />
        <rect x="7" y="55" width="6" height="6" fill="#c0c0c8" />
        <rect x="107" y="11" width="6" height="6" fill="#c0c0c8" />
        <rect x="107" y="55" width="6" height="6" fill="#c0c0c8" />
        <rect x="29" y="35" width="6" height="6" fill="#c0c0c8" />
        <rect x="85" y="35" width="6" height="6" fill="#c0c0c8" />

        {/* Steel blue power core at apex */}
        <rect x="55" y="6" width="10" height="10" fill="#4488cc" />
        <rect x="57" y="8" width="6" height="6" fill="#6699dd" opacity="0.6" />

        {/* Circuit traces — horizontal extensions */}
        <line x1="2" y1="14" x2="10" y2="14" stroke="#4488cc" strokeWidth="1.5" />
        <line x1="110" y1="14" x2="118" y2="14" stroke="#4488cc" strokeWidth="1.5" />
        <rect x="0" y="12" width="4" height="4" fill="#4488cc" opacity="0.6" />
        <rect x="116" y="12" width="4" height="4" fill="#4488cc" opacity="0.6" />

        {/* Dashed line through center */}
        <line
          x1="4"
          y1="36"
          x2="116"
          y2="36"
          stroke="#c0c0c8"
          strokeWidth="1"
          strokeDasharray="4 6"
          opacity="0.3"
        />
      </g>
    </svg>
  );
}
