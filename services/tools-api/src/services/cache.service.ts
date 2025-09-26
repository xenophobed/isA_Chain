import { logger } from '../utils/logger';

export class CacheService {
    private cache: Map<string, any> = new Map();
    private static instance: CacheService;

    private constructor() {}

    static async getInstance(): Promise<CacheService> {
        if (!CacheService.instance) {
            CacheService.instance = new CacheService();
            await CacheService.instance.initialize();
        }
        return CacheService.instance;
    }

    private async initialize() {
        logger.info('Cache service initialized with in-memory storage');
    }

    async get(key: string): Promise<any> {
        return this.cache.get(key);
    }

    async set(key: string, value: any, ttl?: number): Promise<void> {
        this.cache.set(key, {
            value,
            expires: ttl ? Date.now() + ttl * 1000 : null
        });

        if (ttl) {
            setTimeout(() => {
                this.cache.delete(key);
            }, ttl * 1000);
        }
    }

    async delete(key: string): Promise<void> {
        this.cache.delete(key);
    }

    async clear(): Promise<void> {
        this.cache.clear();
    }

    async exists(key: string): Promise<boolean> {
        const entry = this.cache.get(key);
        if (!entry) return false;
        
        if (entry.expires && entry.expires < Date.now()) {
            this.cache.delete(key);
            return false;
        }
        
        return true;
    }

    async keys(pattern?: string): Promise<string[]> {
        const allKeys = Array.from(this.cache.keys());
        
        if (!pattern) {
            return allKeys;
        }

        const regex = new RegExp(pattern.replace(/\*/g, '.*'));
        return allKeys.filter(key => regex.test(key));
    }
}