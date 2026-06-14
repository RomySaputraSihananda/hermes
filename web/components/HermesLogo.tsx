export default function HermesLogo({ size = 28 }: { size?: number }) {
  const id = `hermes-logo-${size}`;
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <defs>
        <linearGradient id={`${id}-g`} x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%"   stopColor="#f0c060" />
          <stop offset="100%" stopColor="#c49833" />
        </linearGradient>
      </defs>
      {/* Stylised H with peaked top (wings) */}
      <path
        d="M 8 27 L 8 11 L 16 5 L 24 11 L 24 27"
        stroke={`url(#${id}-g)`}
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <line x1="8" y1="19" x2="24" y2="19" stroke={`url(#${id}-g)`} strokeWidth="2.4" strokeLinecap="round" />
      {/* Wing tips */}
      <path d="M 4 11 Q 8 8 11 12"  stroke={`url(#${id}-g)`} strokeWidth="1.6" strokeLinecap="round" fill="none" />
      <path d="M 28 11 Q 24 8 21 12" stroke={`url(#${id}-g)`} strokeWidth="1.6" strokeLinecap="round" fill="none" />
    </svg>
  );
}
