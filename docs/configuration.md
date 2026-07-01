# Configuration

# Copy environment config
cp .env.example .env

## 🔧 Configuration

Each service uses environment variables for configuration. See `.env.example` in each service directory.

### Common Configuration

```env

#### TypeScript配置 (tsconfig.json)

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "**/*.test.ts"]
}
```

## References

- [architecture/service-implementation-guide.md](./architecture/service-implementation-guide.md)
- [services-readme.md](./services-readme.md)
- [services/defi-api/README.md](./services/defi-api/README.md)
