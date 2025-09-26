import { Request, Response, NextFunction } from 'express';
import { logger } from '../utils/logger';

export interface AppError extends Error {
    statusCode: number;
    isOperational: boolean;
}

export const createError = (message: string, statusCode: number = 500): AppError => {
    const error = new Error(message) as AppError;
    error.statusCode = statusCode;
    error.isOperational = true;
    return error;
};

export const errorHandler = (
    error: AppError,
    req: Request,
    res: Response,
    next: NextFunction
) => {
    const { statusCode = 500, message = 'Internal Server Error' } = error;
    
    logger.error(`Error ${statusCode}: ${message}`, {
        url: req.url,
        method: req.method,
        stack: error.stack,
    });

    res.status(statusCode).json({
        success: false,
        error: message,
        ...(process.env.NODE_ENV === 'development' && { stack: error.stack }),
    });
};

export const notFoundHandler = (req: Request, res: Response, next: NextFunction) => {
    const error = createError(`Route ${req.originalUrl} not found`, 404);
    next(error);
};

export const asyncHandler = (fn: Function) => (req: Request, res: Response, next: NextFunction) => {
    Promise.resolve(fn(req, res, next)).catch(next);
};