import { Request, Response, NextFunction } from 'express';
import { createError } from './error.middleware';

export const validateRequired = (fields: string[]) => {
    return (req: Request, res: Response, next: NextFunction) => {
        const missing = fields.filter(field => !req.body[field]);
        
        if (missing.length > 0) {
            return next(createError(`Missing required fields: ${missing.join(', ')}`, 400));
        }
        
        next();
    };
};

export const validateEthereumAddress = (field: string) => {
    return (req: Request, res: Response, next: NextFunction) => {
        const address = req.body[field] || req.params[field];
        
        if (!address) {
            return next(createError(`${field} is required`, 400));
        }
        
        // Basic Ethereum address validation (0x + 40 hex chars)
        const ethAddressRegex = /^0x[a-fA-F0-9]{40}$/;
        
        if (!ethAddressRegex.test(address)) {
            return next(createError(`Invalid Ethereum address: ${field}`, 400));
        }
        
        next();
    };
};

export const validatePositiveNumber = (field: string) => {
    return (req: Request, res: Response, next: NextFunction) => {
        const value = req.body[field];
        
        if (value === undefined || value === null) {
            return next(createError(`${field} is required`, 400));
        }
        
        const num = parseFloat(value);
        
        if (isNaN(num) || num <= 0) {
            return next(createError(`${field} must be a positive number`, 400));
        }
        
        next();
    };
};

export const validateTokenId = (req: Request, res: Response, next: NextFunction) => {
    const tokenId = req.params.tokenId || req.body.tokenId;
    
    if (!tokenId) {
        return next(createError('Token ID is required', 400));
    }
    
    if (!/^\d+$/.test(tokenId)) {
        return next(createError('Token ID must be a valid number', 400));
    }
    
    next();
};

export const validateFileUpload = (req: Request, res: Response, next: NextFunction) => {
    if (!req.file && !req.files) {
        return next(createError('No file uploaded', 400));
    }
    
    // Add file size validation if needed
    const maxSize = 10 * 1024 * 1024; // 10MB
    const file = req.file || (Array.isArray(req.files) ? req.files[0] : req.files);
    
    if (file && file.size > maxSize) {
        return next(createError('File too large. Maximum size is 10MB', 400));
    }
    
    next();
};

export const validateRequest = (validationType: string) => {
    return (req: Request, res: Response, next: NextFunction) => {
        switch (validationType) {
            case 'createCollection':
                return validateRequired(['name', 'symbol'])(req, res, next);
            case 'mintNFT':
                return validateRequired(['to', 'name', 'description'])(req, res, next);
            case 'batchMint':
                return validateRequired(['to', 'tokens'])(req, res, next);
            case 'lazyMint':
                return validateRequired(['to', 'tokenURI'])(req, res, next);
            case 'transferNFT':
                return validateRequired(['from', 'to', 'tokenId'])(req, res, next);
            case 'burnNFT':
                return validateRequired(['tokenId'])(req, res, next);
            case 'approveNFT':
                return validateRequired(['to', 'tokenId'])(req, res, next);
            case 'updateMetadata':
                return validateRequired(['tokenId', 'tokenURI'])(req, res, next);
            case 'createListing':
                return validateRequired(['tokenId', 'price'])(req, res, next);
            case 'cancelListing':
                return validateRequired(['listingId'])(req, res, next);
            case 'updatePrice':
                return validateRequired(['listingId', 'newPrice'])(req, res, next);
            case 'buyNFT':
                return validateRequired(['listingId'])(req, res, next);
            case 'makeOffer':
                return validateRequired(['tokenId', 'price'])(req, res, next);
            case 'acceptOffer':
                return validateRequired(['offerId'])(req, res, next);
            case 'rejectOffer':
                return validateRequired(['offerId'])(req, res, next);
            case 'createAuction':
                return validateRequired(['tokenId', 'startingPrice', 'duration'])(req, res, next);
            case 'placeBid':
                return validateRequired(['auctionId', 'amount'])(req, res, next);
            case 'endAuction':
                return validateRequired(['auctionId'])(req, res, next);
            case 'setRoyalty':
                return validateRequired(['collection', 'percentage'])(req, res, next);
            case 'claimRoyalties':
                return validateRequired(['collection'])(req, res, next);
            default:
                next();
        }
    };
};