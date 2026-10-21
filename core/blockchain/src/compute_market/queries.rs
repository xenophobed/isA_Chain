use crate::compute_market::types::*;
use crate::types::*;

impl ComputeMarketState {
    pub(crate) fn has_capacity(available: &ComputeCapacity, required: &ComputeCapacity) -> bool {
        available.cpu_millicores >= required.cpu_millicores
            && available.memory_mb >= required.memory_mb
            && available.storage_gb >= required.storage_gb
            && available.gpu_memory_mb >= required.gpu_memory_mb
    }

    pub(crate) fn deduct_capacity(available: &mut ComputeCapacity, used: &ComputeCapacity) {
        available.cpu_millicores = available.cpu_millicores.saturating_sub(used.cpu_millicores);
        available.memory_mb = available.memory_mb.saturating_sub(used.memory_mb);
        available.storage_gb = available.storage_gb.saturating_sub(used.storage_gb);
        available.gpu_memory_mb = available.gpu_memory_mb.saturating_sub(used.gpu_memory_mb);
    }

    pub(crate) fn restore_capacity(available: &mut ComputeCapacity, released: &ComputeCapacity) {
        available.cpu_millicores += released.cpu_millicores;
        available.memory_mb += released.memory_mb;
        available.storage_gb += released.storage_gb;
        available.gpu_memory_mb += released.gpu_memory_mb;
    }

    /// Get provider by address
    pub fn get_provider(&self, address: &Address) -> Option<&ProviderInfo> {
        self.providers.get(address)
    }

    /// List active providers
    pub fn list_active_providers(&self) -> Vec<&ProviderInfo> {
        self.providers
            .values()
            .filter(|p| p.status == ProviderStatus::Active)
            .collect()
    }

    /// List providers by resource type
    pub fn list_providers_by_resource(&self, resource_type: &ResourceType) -> Vec<&ProviderInfo> {
        self.providers
            .values()
            .filter(|p| {
                p.status == ProviderStatus::Active && p.resource_types.contains(resource_type)
            })
            .collect()
    }

    /// Get job by ID
    pub fn get_job(&self, job_id: &Hash) -> Option<&ComputeJob> {
        self.jobs.get(job_id)
    }

    /// List jobs for user
    pub fn list_user_jobs(&self, user: &Address) -> Vec<&ComputeJob> {
        self.user_jobs
            .get(user)
            .map(|ids| ids.iter().filter_map(|id| self.jobs.get(id)).collect())
            .unwrap_or_default()
    }

    /// List jobs for provider
    pub fn list_provider_jobs(&self, provider: &Address) -> Vec<&ComputeJob> {
        self.provider_jobs
            .get(provider)
            .map(|ids| ids.iter().filter_map(|id| self.jobs.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find matching providers for a job request
    pub fn find_matching_providers(
        &self,
        resource_type: &ResourceType,
        capacity: &ComputeCapacity,
        max_price: Amount,
    ) -> Vec<&ProviderInfo> {
        self.providers
            .values()
            .filter(|p| {
                p.status == ProviderStatus::Active
                    && p.resource_types.contains(resource_type)
                    && p.price_per_hour <= max_price
                    && Self::has_capacity(&p.available_capacity, capacity)
            })
            .collect()
    }
}
