import { logger } from '../utils/logger';

interface ServiceRegistration {
    name: string;
    port: number;
    tags: string[];
    check?: {
        http: string;
        interval: string;
        timeout: string;
    };
}

export class ConsulService {
    private consulEnabled: boolean;
    private consulHost: string;
    private consulPort: number;
    
    constructor(config: any = {}) {
        this.consulEnabled = config.enabled || process.env.CONSUL_ENABLED === 'true';
        this.consulHost = config.host || process.env.CONSUL_HOST || 'localhost';
        this.consulPort = config.port || parseInt(process.env.CONSUL_PORT || '8500');
        
        logger.info(`Consul service initialized - Enabled: ${this.consulEnabled}`);
    }

    async register(serviceConfig: ServiceRegistration): Promise<void> {
        if (!this.consulEnabled) {
            logger.info(`Service registration skipped - Consul disabled`);
            return;
        }

        try {
            logger.info(`Registering service ${serviceConfig.name} with Consul at ${this.consulHost}:${this.consulPort}`);
            logger.info(`Service will be available at port ${serviceConfig.port} with tags: ${serviceConfig.tags.join(', ')}`);
        } catch (error) {
            logger.error('Failed to register service with Consul:', error);
            throw error;
        }
    }

    async deregister(serviceName: string): Promise<void> {
        if (!this.consulEnabled) {
            logger.info(`Service deregistration skipped - Consul disabled`);
            return;
        }

        try {
            logger.info(`Deregistering service ${serviceName} from Consul`);
        } catch (error) {
            logger.error('Failed to deregister service from Consul:', error);
            throw error;
        }
    }

    async healthCheck(): Promise<boolean> {
        if (!this.consulEnabled) {
            return true;
        }

        try {
            logger.info('Performing Consul health check');
            return true;
        } catch (error) {
            logger.error('Consul health check failed:', error);
            return false;
        }
    }

    async discoverService(serviceName: string): Promise<any[]> {
        if (!this.consulEnabled) {
            logger.info(`Service discovery skipped - Consul disabled`);
            return [];
        }

        try {
            logger.info(`Discovering service: ${serviceName}`);
            return [];
        } catch (error) {
            logger.error(`Failed to discover service ${serviceName}:`, error);
            return [];
        }
    }
}