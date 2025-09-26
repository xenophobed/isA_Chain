import Consul from 'consul';
import { logger } from '../utils/logger';

export class ConsulService {
  private consul: Consul.Consul;
  private serviceId: string = '';

  constructor(config: any) {
    this.consul = new Consul({
      host: config.host,
      port: config.port,
      secure: config.secure
    });
  }

  async register(options: any) {
    try {
      this.serviceId = `${options.name}-${Date.now()}`;
      
      await this.consul.agent.service.register({
        id: this.serviceId,
        name: options.name,
        address: 'localhost',
        port: options.port,
        tags: options.tags,
        check: options.check
      });
      
      logger.info(`Service registered with Consul: ${this.serviceId}`);
    } catch (error) {
      logger.warn('Failed to register with Consul (service will work without it):', error);
    }
  }

  async deregister(serviceName: string) {
    try {
      if (this.serviceId) {
        await this.consul.agent.service.deregister(this.serviceId);
        logger.info(`Service deregistered from Consul: ${this.serviceId}`);
      }
    } catch (error) {
      logger.warn('Failed to deregister from Consul:', error);
    }
  }
}